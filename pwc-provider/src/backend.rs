//! The wallet behind the Provider: inventory, chain data, keys and broadcast.

use serde::Serialize;

use crate::types::{Id, ScriptType, Text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletKind {
    Software,
    Hardware,
    Multisig,
    WatchOnly,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletState {
    Ready,
    Locked,
    Syncing,
    Unavailable,
}

/// A wallet with all of its accounts, before any grant filtering.
#[derive(Debug, Clone)]
pub struct WalletInfo {
    pub wallet_id: Id,
    pub display_name: Text,
    pub network: String,
    pub kind: WalletKind,
    pub state: WalletState,
    pub account_ids: Vec<Id>,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub account_id: Id,
    pub wallet_id: Id,
    pub display_name: Text,
    pub network: String,
    pub can_sign: bool,
    pub script_type: ScriptType,
    /// Account-scoped methods usable with this account (Core §4.2).
    pub methods: Vec<&'static str>,
}

pub trait WalletBackend: Send + Sync + 'static {
    fn wallets(&self) -> Vec<WalletInfo>;
    fn account(&self, account_id: &str) -> Option<AccountInfo>;

    fn wallet(&self, wallet_id: &str) -> Option<WalletInfo> {
        self.wallets().into_iter().find(|w| w.wallet_id.as_str() == wallet_id)
    }
}
