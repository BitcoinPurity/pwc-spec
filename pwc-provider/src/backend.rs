//! The wallet behind the Provider: inventory, chain data, keys and broadcast.

use std::collections::HashSet;

use serde::Serialize;

use crate::error::PwcError;
use crate::types::{Hash256, Hex, Id, MessageFormat, Sats, ScriptType, SignedSats, Text, Timestamp};

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

/// A change in the wallet that the host reports to the Provider with `Provider::notify` (Core §8.6).
#[derive(Debug, Clone)]
pub enum BackendEvent {
    WalletLocked {
        wallet_id: Id,
    },
    WalletUnlocked {
        wallet_id: Id,
    },
    WalletSyncChanged {
        wallet_id: Id,
    },
    BalanceChanged {
        account_id: Id,
    },
    /// `status` as in `TransactionRecord`.
    TransactionChanged {
        account_id: Id,
        txid: String,
        status: &'static str,
    },
}

/// Outpoints reserved by PSBT requests; reported as `locked` (Core §8.3).
pub type Reserved = HashSet<bitcoin::OutPoint>;

/// A payment request for `psbt.create`, with addresses already checked against the account's network.
#[derive(Debug, Clone)]
pub struct CreateRequest {
    pub outputs: Vec<(bitcoin::ScriptBuf, u64)>,
    /// sat/kvB
    pub fee_rate: u64,
    pub selected: Option<Vec<bitcoin::OutPoint>>,
    pub replaceable: bool,
}

#[derive(Debug, Clone)]
pub struct CreatedPsbt {
    pub psbt: bitcoin::Psbt,
    pub fee: u64,
    pub vsize: u64,
    pub change_output_indexes: Vec<u32>,
    pub inputs: Vec<bitcoin::OutPoint>,
}

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
    fn owns_script(&self, account_id: &str, script: &bitcoin::Script) -> bool;
    /// Whether the outpoint is an effective unspent output of the account.
    fn is_unspent(&self, account_id: &str, outpoint: &bitcoin::OutPoint) -> bool;
    /// The wallet's fee estimate in sat/kvB, `None` if it cannot compute one now.
    fn fee_estimate(&self, network: &str, target_blocks: u64) -> Option<u64>;
    /// Selects coins and builds an unsigned PSBT; `excluded` outpoints are not selected.
    fn create_psbt(
        &self,
        account_id: &str,
        request: &CreateRequest,
        excluded: &Reserved,
    ) -> Result<CreatedPsbt, PwcError>;
    /// The account's output at `outpoint` on the account's network, if the account owns it.
    fn owned_output(&self, account_id: &str, outpoint: &bitcoin::OutPoint) -> Option<bitcoin::TxOut>;
    /// Adds the account's `SIGHASH_ALL` signatures to the given inputs, whose `witness_utxo` is set; returns the
    /// indexes that gained a signature.
    fn sign_psbt(&self, account_id: &str, psbt: &mut bitcoin::Psbt, inputs: &[usize]) -> Result<Vec<u32>, PwcError>;
    /// Signs the exact UTF-8 bytes of `message` with the account key of `script`.
    fn sign_message(
        &self,
        account_id: &str,
        script: &bitcoin::Script,
        message: &str,
        format: MessageFormat,
    ) -> Result<Vec<u8>, PwcError>;
    /// Submits a transaction to the network backend; `TRANSACTION_REJECTED` or `NETWORK_UNAVAILABLE` on failure.
    fn broadcast(&self, network: &str, tx: &bitcoin::Transaction) -> Result<bitcoin::Txid, PwcError>;

    fn wallet(&self, wallet_id: &str) -> Option<WalletInfo> {
        self.wallets().into_iter().find(|w| w.wallet_id.as_str() == wallet_id)
    }
}
