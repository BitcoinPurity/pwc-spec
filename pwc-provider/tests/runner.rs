//! Runs the LOCAL conformance runner of `pwc-testkit/` against the demo binary. The runner opens AF_UNIX
//! sockets only, so this test runs on macOS and Linux.
#![cfg(unix)]

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn local_runner_reports_no_failures() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("run");
    let mut provider = Command::new(env!("CARGO_BIN_EXE_pwc-provider"))
        .args([
            "--dir",
            run.to_str().unwrap(),
            "--approve",
            "auto",
            "--approve-delay-ms",
            "300",
        ])
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let socket = run.join("pwc-provider.sock");
    let start = Instant::now();
    while !socket.exists() {
        assert!(start.elapsed() < Duration::from_secs(10), "Provider did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let report = dir.path().join("report.json");
    let runner = format!("{}/../pwc-testkit/local_runner.py", env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("python3")
        .args([
            runner.as_str(),
            "--socket",
            socket.to_str().unwrap(),
            "--pair",
            "--allow-destructive",
            "--report",
        ])
        .arg(&report)
        .output()
        .unwrap();
    provider.kill().unwrap();
    let _ = provider.wait();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "runner failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    let outcomes: Vec<(&str, &str)> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["id"].as_str().unwrap(), r["outcome"].as_str().unwrap()))
        .collect();
    assert!(
        outcomes.iter().all(|(_, o)| *o == "pass" || *o == "manual"),
        "{outcomes:?}"
    );
    assert_eq!(outcomes.iter().filter(|(_, o)| *o == "pass").count(), 16, "{stdout}");
}
