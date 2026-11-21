//! The wallet behind the Provider: inventory, chain data, keys and broadcast.

use std::collections::HashSet;

use serde::Serialize;

use crate::error::PwcError;
use crate::types::{Hash256, Hex, Id, Sats, ScriptType, SignedSats, Text, Timestamp};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncStatus {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_height: Option<u32>,
}

/// Core §4.3 `Balance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Balance {
    pub account_id: Id,
    pub network: String,
    #[serde(flatten)]
    pub components: crate::balance::Components,
    pub updated_at: Timestamp,
    pub sync: SyncStatus,
}

/// Core §4.4 `AddressRecord`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressRecord {
    pub account_id: Id,
    pub network: String,
    pub address: String,
    pub role: &'static str,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Text>,
}

/// Core §4.5 `Utxo`, built from an explicit field list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Utxo {
    pub txid: Hash256,
    pub vout: u32,
    pub account_id: Id,
    pub network: String,
    pub value_sats: Sats,
    pub confirmations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    pub address: String,
    pub script_pub_key: Hex,
    pub spendable: bool,
    pub restrictions: Vec<&'static str>,
    pub is_change: bool,
}

/// Core §4.6 `TransactionRecord`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionRecord {
    pub account_id: Id,
    pub network: String,
    pub txid: Hash256,
    pub status: &'static str,
    pub confirmations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<Timestamp>,
    pub net_change_sats: SignedSats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_sats: Option<Sats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_by_txid: Option<Hash256>,
}

/// Outpoints reserved by PSBT requests; reported as `locked` (Core §8.3).
pub type Reserved = HashSet<bitcoin::OutPoint>;

/// Account data methods are called only for existing accounts of readable wallets (checks 7 and 8).
pub trait WalletBackend: Send + Sync + 'static {
    fn wallets(&self) -> Vec<WalletInfo>;
    fn account(&self, account_id: &str) -> Option<AccountInfo>;
    fn balance(&self, account_id: &str, reserved: &Reserved) -> Result<Balance, PwcError>;
    /// The current receive address; repeated calls may return the same one.
    fn receive_address(&self, account_id: &str) -> Result<AddressRecord, PwcError>;
    fn create_address(&self, account_id: &str, label: Option<&Text>) -> Result<AddressRecord, PwcError>;
    fn addresses(&self, account_id: &str) -> Vec<AddressRecord>;
    fn utxos(&self, account_id: &str, reserved: &Reserved) -> Result<Vec<Utxo>, PwcError>;
    /// Newest first.
    fn transactions(&self, account_id: &str) -> Result<Vec<TransactionRecord>, PwcError>;

    fn wallet(&self, wallet_id: &str) -> Option<WalletInfo> {
        self.wallets().into_iter().find(|w| w.wallet_id.as_str() == wallet_id)
    }
}
