//! Demo PWC LOCAL Provider backed by the in-memory wallet.
//!
//! Usage: pwc-provider [--dir DIR] [--approve ask|auto] [--approve-delay-ms N]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use pwc_provider::backend::WalletBackend;
use pwc_provider::local;
use pwc_provider::memory::MemoryBackend;
use pwc_provider::prompt::{GrantChoice, PermissionRequest, Prompt};
use pwc_provider::provider::{Config, PromptReceiver, Provider};
use pwc_provider::types::Network;
use tokio::io::{AsyncBufReadExt, BufReader};

struct Args {
    dir: Option<PathBuf>,
    auto: bool,
    delay: Duration,
}

fn usage() -> ! {
    eprintln!("usage: pwc-provider [--dir DIR] [--approve ask|auto] [--approve-delay-ms N]");
    std::process::exit(2)
}

fn args() -> Args {
    let mut a = Args {
        dir: None,
        auto: false,
        delay: Duration::from_millis(300),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--dir" => a.dir = Some(PathBuf::from(value)),
            "--approve" => match value.as_str() {
                "ask" => a.auto = false,
                "auto" => a.auto = true,
                _ => usage(),
            },
            "--approve-delay-ms" => a.delay = Duration::from_millis(value.parse().unwrap_or_else(|_| usage())),
            _ => usage(),
        }
    }
    a
}

/// Grants what was requested: the requested network or `regtest`, and the requested wallets and accounts or
/// every wallet of that network.
fn as_requested(backend: &dyn WalletBackend, r: &PermissionRequest) -> GrantChoice {
    let network = r.network.clone().unwrap_or_else(|| Network("regtest".into()));
    let (wallet_ids, account_ids) = if r.wallet_ids.is_empty() && r.account_ids.is_empty() {
        let all = backend
            .wallets()
            .into_iter()
            .filter(|w| w.network == network.as_str())
            .map(|w| w.wallet_id)
            .collect();
        (all, Vec::new())
    } else {
        (r.wallet_ids.clone(), r.account_ids.clone())
    };
    GrantChoice {
        network,
        scopes: r.scopes.clone(),
        wallet_ids,
        account_ids,
        expires_at: r.expires_at,
        constraints: r.constraints.clone(),
    }
}

fn answer(backend: &dyn WalletBackend, prompt: Prompt, yes: bool) {
    match prompt {
        Prompt::Pairing { reply, .. } => {
            let _ = reply.send(yes);
        }
        Prompt::Permissions { request, reply } => {
            let _ = reply.send(yes.then(|| as_requested(backend, &request)));
        }
        Prompt::SignPsbt { reply, .. } | Prompt::SignMessage { reply, .. } => {
            let _ = reply.send(yes);
        }
    }
}

async fn approve_automatically(backend: Arc<MemoryBackend>, mut prompts: PromptReceiver, delay: Duration) {
    while let Some(prompt) = prompts.recv().await {
        eprintln!("[auto-approve]\n{prompt}");
        let backend = backend.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            answer(&*backend, prompt, true);
        });
    }
}

async fn ask(backend: Arc<MemoryBackend>, mut prompts: PromptReceiver) {
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    while let Some(prompt) = prompts.recv().await {
        eprintln!("\n{prompt}\nApprove? [y/N] ");
        let yes = matches!(stdin.next_line().await, Ok(Some(l)) if l.trim().eq_ignore_ascii_case("y"));
        answer(&*backend, prompt, yes);
    }
}

#[tokio::main]
async fn main() {
    let a = args();
    let dir = a.dir.or_else(local::default_directory).unwrap_or_else(|| {
        eprintln!("cannot determine the endpoint directory; pass --dir");
        std::process::exit(1)
    });
    let backend = Arc::new(MemoryBackend::demo());
    let (provider, prompts) = Provider::new(backend.clone(), Config::default());
    let endpoint = match local::bind(&dir, pwc_provider::PROVIDER_ID) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("pwc-provider: {e}");
            std::process::exit(1)
        }
    };
    eprintln!("pwc-provider listening on {}", endpoint.path().display());
    if a.auto {
        eprintln!("WARNING: --approve auto approves every pairing and request without asking. Test use only.");
        tokio::spawn(approve_automatically(backend, prompts, a.delay));
    } else {
        tokio::spawn(ask(backend, prompts));
    }
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = local::serve(provider, endpoint) => {}
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}
