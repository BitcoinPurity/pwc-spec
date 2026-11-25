//! Wire types of Core v0.2.1 §3 and §4, and method parameters and results of §8.
//!
//! Amounts and fee rates are integers here and canonical decimal strings on the wire. Timestamps, hashes, hex,
//! base64, addresses, identifiers and cursors keep their wire form as strings.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, PwcError};

pub const MAX_MONEY: u64 = 2_100_000_000_000_000;
pub const MAX_FEE_RATE: u64 = 1_000_000_000;

/// Parses `0` or `[1-9][0-9]*`.
fn canonical(s: &str) -> Option<u64> {
    let ok = s == "0" || (!s.starts_with('0') && !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    if ok {
        s.parse().ok()
    } else {
        None
    }
}

fn parse_sats(s: &str) -> Option<u64> {
    canonical(s).filter(|v| *v <= MAX_MONEY)
}

fn parse_positive_sats(s: &str) -> Option<u64> {
    parse_sats(s).filter(|v| *v > 0)
}

fn parse_fee_rate(s: &str) -> Option<u64> {
    canonical(s).filter(|v| (1..=MAX_FEE_RATE).contains(v))
}

fn parse_signed_sats(s: &str) -> Option<i64> {
    match s.strip_prefix('-') {
        Some(m) => parse_positive_sats(m).map(|v| -(v as i64)),
        None => parse_sats(s).map(|v| v as i64),
    }
}

/// Defines a module for `#[serde(with = ...)]` that writes an integer as a decimal string, and `option` inside
/// it for optional fields.
macro_rules! decimal_string {
    ($name:ident, $int:ty, $parse:ident, $what:literal) => {
        pub mod $name {
            use serde::{Deserialize, Deserializer, Serializer};

            pub fn serialize<S: Serializer>(v: &$int, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(v)
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<$int, D::Error> {
                let s = String::deserialize(d)?;
                super::$parse(&s).ok_or_else(|| serde::de::Error::custom(concat!("invalid ", $what)))
            }

            pub mod option {
                use serde::{Deserializer, Serializer};

                pub fn serialize<S: Serializer>(v: &Option<$int>, s: S) -> Result<S::Ok, S::Error> {
                    match v {
                        Some(v) => super::serialize(v, s),
                        None => s.serialize_none(),
                    }
                }

                pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<$int>, D::Error> {
                    super::deserialize(d).map(Some)
                }
            }
        }
    };
}

decimal_string!(sats, u64, parse_sats, "Sats");
decimal_string!(positive_sats, u64, parse_positive_sats, "PositiveSats");
decimal_string!(signed_sats, i64, parse_signed_sats, "SignedSats");
decimal_string!(fee_rate, u64, parse_fee_rate, "FeeRate");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "kebab-case")]
pub enum ScriptType {
    P2pkh,
    P2sh,
    P2shP2wpkh,
    P2shP2wsh,
    P2wpkh,
    P2wsh,
    P2tr,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "kebab-case")]
pub enum MessageFormat {
    Bip322Simple,
    Bip322Full,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct OutPoint {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Synced,
    Syncing,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SyncStatus {
    pub state: SyncState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeePolicy {
    /// A fee rate in sat/kvB: 1250 is 1.25 sat/vB.
    Rate {
        #[serde(with = "fee_rate")]
        sat_per_kvb: u64,
    },
    WalletEstimate {
        target_blocks: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Desktop,
    Mobile,
    Extension,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ProviderInfo {
    pub name: String,
    pub version: String,
    pub platform: Platform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Limits {
    pub max_page_size: u64,
    pub max_psbt_bytes: u64,
    pub max_combine_psbts: u64,
    pub max_raw_transaction_bytes: u64,
    pub max_message_bytes: u64,
    pub max_concurrent_operations: u64,
    pub operation_retention_seconds: u64,
    #[serde(with = "fee_rate")]
    pub high_fee_rate: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Capabilities {
    pub pwc_version: String,
    pub provider: ProviderInfo,
    pub profiles: Vec<String>,
    pub supported_networks: Vec<String>,
    pub methods: Vec<String>,
    pub features: Vec<String>,
    pub scopes: Vec<String>,
    pub script_types: Vec<ScriptType>,
    pub message_formats: Vec<MessageFormat>,
    pub limits: Limits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum WalletKind {
    Software,
    Hardware,
    Multisig,
    WatchOnly,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum WalletState {
    Ready,
    Locked,
    Syncing,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct WalletSummary {
    pub wallet_id: String,
    pub display_name: String,
    pub network: String,
    pub wallet_kind: WalletKind,
    pub state: WalletState,
    pub account_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AccountSummary {
    pub account_id: String,
    pub wallet_id: String,
    pub display_name: String,
    pub network: String,
    pub can_sign: bool,
    pub script_types: Vec<ScriptType>,
    pub methods: Vec<String>,
}

/// Core §4.3. The effective value is `confirmed_sats + unconfirmed_delta_sats`; show `spendable_sats` as the
/// amount available to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Balance {
    pub account_id: String,
    pub network: String,
    #[serde(with = "sats")]
    pub confirmed_sats: u64,
    #[serde(with = "sats")]
    pub pending_self_sats: u64,
    #[serde(with = "sats")]
    pub pending_external_sats: u64,
    #[serde(with = "sats")]
    pub pending_outgoing_sats: u64,
    #[serde(with = "signed_sats")]
    pub unconfirmed_delta_sats: i64,
    #[serde(with = "sats")]
    pub spendable_sats: u64,
    #[serde(with = "sats")]
    pub locked_sats: u64,
    #[serde(with = "sats")]
    pub immature_sats: u64,
    pub updated_at: String,
    pub sync: SyncStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum AddressRole {
    Receive,
    Change,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum AddressState {
    Unused,
    Used,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AddressRecord {
    pub account_id: String,
    pub network: String,
    pub address: String,
    pub role: AddressRole,
    pub state: AddressState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum UtxoRestriction {
    Frozen,
    Locked,
    Immature,
    Unconfirmed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub account_id: String,
    pub network: String,
    #[serde(with = "sats")]
    pub value_sats: u64,
    pub confirmations: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_pub_key: Option<String>,
    pub spendable: bool,
    pub restrictions: Vec<UtxoRestriction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_change: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Unconfirmed,
    Confirmed,
    Conflicted,
    Replaced,
    Dropped,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct TransactionRecord {
    pub account_id: String,
    pub network: String,
    pub txid: String,
    pub status: TransactionStatus,
    pub confirmations: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_seen_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<String>,
    #[serde(with = "signed_sats")]
    pub net_change_sats: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "sats::option")]
    pub fee_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by_txid: Option<String>,
}

/// The result of `psbt.combine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PsbtObject {
    pub psbt: String,
    pub psbt_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CreatePsbtResult {
    pub psbt: String,
    pub psbt_version: u32,
    pub account_id: String,
    pub network: String,
    #[serde(with = "sats")]
    pub fee_sats: u64,
    pub vsize: u64,
    pub change_output_indexes: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserved_until: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum SigningStatus {
    Partial,
    Complete,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SignPsbtResult {
    pub psbt: String,
    pub psbt_version: u32,
    pub account_id: String,
    pub network: String,
    pub signed_input_indexes: Vec<u32>,
    pub signing_status: SigningStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct FinalizePsbtResult {
    pub psbt: String,
    pub psbt_version: u32,
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_transaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub txid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SignMessageResult {
    pub signature: String,
    pub format: MessageFormat,
    pub address: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct GrantConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "positive_sats::option")]
    pub max_amount_sats_per_request: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum GrantStatus {
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Grant {
    pub grant_id: String,
    pub client_id: String,
    pub network: String,
    pub scopes: Vec<String>,
    pub wallet_ids: Vec<String>,
    pub account_ids: Vec<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub status: GrantStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<GrantConstraints>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SessionInfo {
    pub session_id: String,
    pub client_id: String,
    pub grant_ids: Vec<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Queued,
    AwaitingApproval,
    AwaitingUnlock,
    AwaitingSigner,
    Processing,
    Completed,
    Rejected,
    Expired,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Operation {
    pub operation_id: String,
    pub method: String,
    pub state: OperationState,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// The method's result; read it with the accessor for the method, such as `sign_psbt_result`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PwcError>,
}

#[derive(Deserialize)]
struct GrantsResult {
    grants: Vec<Grant>,
}

#[derive(Deserialize)]
struct TxidResult {
    txid: String,
}

impl Operation {
    fn completed<T: DeserializeOwned>(&self) -> Result<T, Error> {
        if let Some(e) = &self.error {
            return Err(Error::Pwc(e.clone()));
        }
        if self.state != OperationState::Completed {
            return Err(Error::OperationPending);
        }
        let result = self.result.clone().ok_or(Error::ResultUnavailable)?;
        serde_json::from_value(result).map_err(|e| Error::Protocol(format!("result of {}: {e}", self.method)))
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Operation {
    /// True in the states `completed`, `rejected`, `expired`, `canceled` and `failed`, which never change again.
    pub fn is_terminal(&self) -> bool {
        use OperationState::*;
        matches!(self.state, Completed | Rejected | Expired | Canceled | Failed)
    }

    /// The grants made by a completed `session.requestPermissions`. They may be fewer than requested.
    pub fn permissions_result(&self) -> Result<Vec<Grant>, Error> {
        self.completed::<GrantsResult>().map(|r| r.grants)
    }

    pub fn create_psbt_result(&self) -> Result<CreatePsbtResult, Error> {
        self.completed()
    }

    pub fn sign_psbt_result(&self) -> Result<SignPsbtResult, Error> {
        self.completed()
    }

    /// The txid of a completed `transaction.broadcast`. Acceptance by the backend is not a confirmation.
    pub fn broadcast_result(&self) -> Result<String, Error> {
        self.completed::<TxidResult>().map(|r| r.txid)
    }

    pub fn sign_message_result(&self) -> Result<SignMessageResult, Error> {
        self.completed()
    }
}

/// Core §4.8 and §8.6. Events are hints; the read methods give the authoritative state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PwcEvent {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AddressPage {
    pub addresses: Vec<AddressRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct UtxoPage {
    pub utxos: Vec<Utxo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct TransactionPage {
    pub transactions: Vec<TransactionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters of `session.requestPermissions` (Core §8.1). Ask only for the scopes the application needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RequestPermissionsParams {
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<GrantConstraints>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Payment {
    pub address: String,
    #[serde(with = "positive_sats")]
    pub amount_sats: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CreatePsbtParams {
    pub account_id: String,
    pub outputs: Vec<Payment>,
    pub fee_policy: FeePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_utxos: Option<Vec<OutPoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaceable: Option<bool>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SignPsbtParams {
    pub account_id: String,
    pub psbt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_indexes: Option<Vec<u32>>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SignMessageParams {
    pub account_id: String,
    pub address: String,
    pub message: String,
    pub format: MessageFormat,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct VerifyMessageParams {
    pub network: String,
    pub address: String,
    pub message: String,
    pub signature: String,
    pub format: MessageFormat,
}

/// The Provider named in a `local.hello` result (LOCAL §5.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LocalProvider {
    pub name: String,
    pub version: String,
}

/// The result of `local.hello` (LOCAL §5.2), without the nonce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct HelloInfo {
    pub protocol: String,
    pub pwc_version: String,
    pub provider_id: String,
    pub provider: LocalProvider,
    pub max_frame_bytes: u64,
    pub delegation: bool,
}

/// Parameters of `local.pair` (LOCAL §5.3). `pairing_code` comes from `pairing_code()` and is shown to the User
/// before `pair` is called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PairParams {
    pub client_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_description: Option<String>,
    pub pairing_code: String,
}
