//! TASK-FFI-002: generates the Python module and runs `tests/test_bindings.py` against the Rust Provider.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use pwc_provider::backend::WalletBackend;
use pwc_provider::memory::MemoryBackend;
use pwc_provider::prompt::{GrantChoice, Prompt};
use pwc_provider::provider::{Config, Provider};
use pwc_provider::types::Network;

fn run(command: &mut Command) {
    let output = command.output().unwrap();
    let (out, err) = (
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.status.success(), "{command:?} failed\n{out}\n{err}");
    print!("{out}");
}

#[tokio::test(flavor = "multi_thread")]
async fn python_module_works_against_the_provider() {
    let dir = tempfile::tempdir().unwrap();
    let (bindings, run_dir) = (dir.path().join("python"), dir.path().join("run"));

    // `cargo test` does not build the shared library, since no test links it. It is built into the directory of
    // the generator program.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run(Command::new(cargo)
        .args(["build", "--lib", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR")));
    let generator = Path::new(env!("CARGO_BIN_EXE_uniffi-bindgen"));
    let name = if cfg!(target_os = "macos") {
        "libpwc_client_ffi.dylib"
    } else {
        "libpwc_client_ffi.so"
    };
    let library = generator.parent().unwrap().join(name);
    run(Command::new(generator)
        .args(["generate", "--language", "python", "--library"])
        .arg(&library)
        .arg("--out-dir")
        .arg(&bindings)
        .current_dir(env!("CARGO_MANIFEST_DIR")));
    std::fs::copy(&library, bindings.join(name)).unwrap();

    let backend = Arc::new(MemoryBackend::demo());
    let (provider, mut prompts) = Provider::new(backend.clone(), Config::default());
    let endpoint = pwc_provider::local::bind(&run_dir, pwc_provider::PROVIDER_ID).unwrap();
    tokio::spawn(pwc_provider::local::serve(provider, endpoint));
    tokio::spawn(async move {
        while let Some(prompt) = prompts.recv().await {
            tokio::time::sleep(Duration::from_millis(200)).await;
            match prompt {
                Prompt::Pairing { reply, .. } | Prompt::SignPsbt { reply, .. } | Prompt::SignMessage { reply, .. } => {
                    let _ = reply.send(true);
                }
                Prompt::Permissions { request, reply } => {
                    let network = request.network.clone().unwrap_or_else(|| Network("regtest".into()));
                    let wallets = backend.wallets().into_iter().filter(|w| w.network == network.as_str());
                    let _ = reply.send(Some(GrantChoice {
                        wallet_ids: wallets.map(|w| w.wallet_id).collect(),
                        network,
                        scopes: request.scopes.clone(),
                        account_ids: Vec::new(),
                        expires_at: request.expires_at,
                        constraints: request.constraints.clone(),
                    }));
                }
            }
        }
    });

    let script = format!("{}/tests/test_bindings.py", env!("CARGO_MANIFEST_DIR"));
    let scratch = dir.path().join("scratch");
    tokio::task::spawn_blocking(move || {
        run(Command::new("python3")
            .arg("-B")
            .arg(script)
            .arg(&bindings)
            .arg(&run_dir)
            .arg(&scratch));
    })
    .await
    .unwrap();
}
