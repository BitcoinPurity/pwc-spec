//! In-memory demo wallet and simulated chains. Keys derive from fixed seeds: test use only.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use crate::backend::{AccountInfo, WalletBackend, WalletInfo, WalletKind, WalletState};
use crate::types::{Id, ScriptType, Text, SUPPORTED_NETWORKS};

/// Account-scoped methods (Core §4.2) by account kind.
const SIGNING_WPKH: &[&str] = &[
    "account.getBalance",
    "address.getReceiveAddress",
    "address.create",
    "address.list",
    "utxo.list",
    "transaction.list",
    "transaction.get",
    "psbt.create",
    "psbt.releaseReservation",
    "psbt.sign",
    "transaction.broadcast",
    "message.sign",
];
const SIGNING_PKH: &[&str] = &[
    "account.getBalance",
    "address.getReceiveAddress",
    "address.create",
    "address.list",
    "utxo.list",
    "transaction.list",
    "transaction.get",
    "transaction.broadcast",
    "message.sign",
];
const WATCH_WPKH: &[&str] = &[
    "account.getBalance",
    "address.getReceiveAddress",
    "address.create",
    "address.list",
    "utxo.list",
    "transaction.list",
    "transaction.get",
    "psbt.create",
    "psbt.releaseReservation",
    "transaction.broadcast",
];

struct Wallet {
    info: WalletInfo,
}

struct Account {
    info: AccountInfo,
}

struct Mem {
    wallets: Vec<Wallet>,
    accounts: BTreeMap<Id, Account>,
}

pub struct MemoryBackend {
    mem: Mutex<Mem>,
}

impl MemoryBackend {
    /// The demo data of `docs/api-contract.md` §6.
    pub fn demo() -> MemoryBackend {
        let backend = MemoryBackend { mem: Mutex::new(Mem { wallets: Vec::new(), accounts: BTreeMap::new() }) };
        for net in SUPPORTED_NETWORKS {
            let main = backend.add_wallet(&format!("w-{net}-main"), "Main wallet", net, WalletKind::Software);
            backend.add_account(&main, &format!("a-{net}-wpkh"), "Savings", ScriptType::P2wpkh, true);
            backend.add_account(&main, &format!("a-{net}-pkh"), "Legacy", ScriptType::P2pkh, true);
            let watch = backend.add_wallet(&format!("w-{net}-watch"), "Watch-only wallet", net, WalletKind::WatchOnly);
            backend.add_account(&watch, &format!("a-{net}-watch"), "Cold storage", ScriptType::P2wpkh, false);
        }
        backend
    }

    fn lock(&self) -> MutexGuard<'_, Mem> {
        self.mem.lock().unwrap()
    }

    pub fn add_wallet(&self, wallet_id: &str, name: &str, network: &str, kind: WalletKind) -> Id {
        let id = Id(wallet_id.into());
        let info = WalletInfo {
            wallet_id: id.clone(),
            display_name: Text(name.into()),
            network: network.into(),
            kind,
            state: WalletState::Ready,
            account_ids: Vec::new(),
        };
        self.lock().wallets.push(Wallet { info });
        id
    }

    pub fn add_account(&self, wallet_id: &Id, account_id: &str, name: &str, script_type: ScriptType, can_sign: bool) -> Id {
        let mut mem = self.lock();
        let wallet = mem.wallets.iter_mut().find(|w| &w.info.wallet_id == wallet_id).expect("known wallet");
        let id = Id(account_id.into());
        wallet.info.account_ids.push(id.clone());
        let methods = match (script_type, can_sign) {
            (ScriptType::P2wpkh, true) => SIGNING_WPKH,
            (ScriptType::P2pkh, true) => SIGNING_PKH,
            (ScriptType::P2wpkh, false) => WATCH_WPKH,
            (ScriptType::P2pkh, false) => &SIGNING_PKH[..7],
        };
        let info = AccountInfo {
            account_id: id.clone(),
            wallet_id: wallet_id.clone(),
            display_name: Text(name.into()),
            network: wallet.info.network.clone(),
            can_sign,
            script_type,
            methods: methods.to_vec(),
        };
        mem.accounts.insert(id.clone(), Account { info });
        id
    }

    pub fn set_wallet_state(&self, wallet_id: &str, state: WalletState) {
        let mut mem = self.lock();
        let wallet = mem.wallets.iter_mut().find(|w| w.info.wallet_id.as_str() == wallet_id).expect("known wallet");
        wallet.info.state = state;
    }
}

impl WalletBackend for MemoryBackend {
    fn wallets(&self) -> Vec<WalletInfo> {
        self.lock().wallets.iter().map(|w| w.info.clone()).collect()
    }

    fn account(&self, account_id: &str) -> Option<AccountInfo> {
        self.lock().accounts.get(account_id).map(|a| a.info.clone())
    }
}
