//! Core §8 method registry and parameter parsing (check 4).

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{err, ErrorCode, PwcError};
use crate::provider::Limits;
use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    None,
    Wallet,
    Account,
}

#[derive(Debug)]
pub struct MethodSpec {
    pub name: &'static str,
    pub scope: Option<&'static str>,
    pub resource: Resource,
    /// Returns an Operation (check 9 applies).
    pub operation: bool,
    pub idempotent: bool,
    /// Reads account data, so the wallet state of check 8 applies.
    pub reads_account_data: bool,
    /// Shows an approval prompt (Core §12, item 13).
    pub prompts: bool,
}

const fn m(
    name: &'static str,
    scope: Option<&'static str>,
    resource: Resource,
    operation: bool,
    idempotent: bool,
    reads_account_data: bool,
    prompts: bool,
) -> MethodSpec {
    MethodSpec {
        name,
        scope,
        resource,
        operation,
        idempotent,
        reads_account_data,
        prompts,
    }
}

use Resource::{Account as A, None as N, Wallet as W};

pub const METHODS: [MethodSpec; 24] = [
    m("pwc.getCapabilities", None, N, false, false, false, false),
    m("session.getInfo", None, N, false, false, false, false),
    m("session.requestPermissions", None, N, true, true, false, true),
    m("session.getPermissions", None, N, false, false, false, false),
    m("session.revokePermissions", None, N, false, true, false, false),
    m("wallet.list", Some("wallets:read"), N, false, false, false, false),
    m("account.list", Some("wallets:read"), W, false, false, true, false),
    m(
        "account.getBalance",
        Some("balances:read"),
        A,
        false,
        false,
        true,
        false,
    ),
    m(
        "address.getReceiveAddress",
        Some("addresses:read"),
        A,
        false,
        false,
        true,
        false,
    ),
    m("address.create", Some("addresses:create"), A, false, true, true, false),
    m("address.list", Some("addresses:read"), A, false, false, true, false),
    m("utxo.list", Some("utxos:read"), A, false, false, true, false),
    m(
        "transaction.list",
        Some("transactions:read"),
        A,
        false,
        false,
        true,
        false,
    ),
    m(
        "transaction.get",
        Some("transactions:read"),
        A,
        false,
        false,
        true,
        false,
    ),
    m("psbt.create", Some("psbt:create"), A, true, true, true, false),
    m(
        "psbt.releaseReservation",
        Some("psbt:create"),
        A,
        false,
        true,
        false,
        false,
    ),
    m("psbt.sign", Some("psbt:sign"), A, true, true, true, true),
    m("psbt.combine", None, N, false, false, false, false),
    m("psbt.finalize", None, N, false, false, false, false),
    m(
        "transaction.broadcast",
        Some("transactions:broadcast"),
        A,
        true,
        true,
        true,
        false,
    ),
    m("message.sign", Some("messages:sign"), A, true, true, true, true),
    m("message.verify", None, N, false, false, false, false),
    m("operation.get", None, N, false, false, false, false),
    m("operation.cancel", None, N, false, true, false, false),
];

pub fn lookup(name: &str) -> Option<&'static MethodSpec> {
    METHODS.iter().find(|m| m.name == name)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestPermissions {
    pub scopes: Vec<Scope>,
    pub network: Option<Network>,
    pub wallet_ids: Option<Vec<Id>>,
    pub account_ids: Option<Vec<Id>>,
    pub expires_at: Option<Timestamp>,
    pub constraints: Option<GrantConstraints>,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokePermissions {
    pub grant_id: Id,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRef {
    pub wallet_id: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRef {
    pub account_id: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressCreate {
    pub account_id: Id,
    pub label: Option<Text>,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listing {
    pub account_id: Id,
    pub cursor: Option<Cursor>,
    pub page_size: Option<PositiveInteger>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionGet {
    pub account_id: Id,
    pub txid: Hash256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payment {
    pub address: Address,
    pub amount_sats: PositiveSats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PsbtCreate {
    pub account_id: Id,
    pub outputs: Vec<Payment>,
    pub fee_policy: FeePolicy,
    pub selected_utxos: Option<Vec<OutPoint>>,
    pub replaceable: Option<bool>,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseReservation {
    pub account_id: Id,
    pub reservation_id: Id,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PsbtSign {
    pub account_id: Id,
    pub psbt: Base64,
    pub input_indexes: Option<Vec<u32>>,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PsbtCombine {
    pub psbts: Vec<Base64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PsbtFinalize {
    pub psbt: Base64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Broadcast {
    pub account_id: Id,
    pub raw_transaction: Hex,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageSign {
    pub account_id: Id,
    pub address: Address,
    pub message: String,
    pub format: MessageFormat,
    pub idempotency_key: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageVerify {
    pub network: Network,
    pub address: Address,
    pub message: String,
    pub signature: Base64,
    pub format: MessageFormat,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRef {
    pub operation_id: Id,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationCancel {
    pub operation_id: Id,
    pub idempotency_key: Id,
}

/// A parsed request.
#[derive(Debug)]
pub enum Request {
    GetCapabilities,
    GetInfo,
    RequestPermissions(RequestPermissions),
    GetPermissions,
    RevokePermissions(RevokePermissions),
    WalletList,
    AccountList(WalletRef),
    GetBalance(AccountRef),
    GetReceiveAddress(AccountRef),
    AddressCreate(AddressCreate),
    AddressList(Listing),
    UtxoList(Listing),
    TransactionList(Listing),
    TransactionGet(TransactionGet),
    PsbtCreate(PsbtCreate),
    ReleaseReservation(ReleaseReservation),
    PsbtSign(PsbtSign),
    PsbtCombine(PsbtCombine),
    PsbtFinalize(PsbtFinalize),
    Broadcast(Broadcast),
    MessageSign(MessageSign),
    MessageVerify(MessageVerify),
    OperationGet(OperationRef),
    OperationCancel(OperationCancel),
}

fn de<T: DeserializeOwned>(v: Value) -> Result<T, PwcError> {
    serde_json::from_value(v).map_err(|e| err(ErrorCode::InvalidArgument, e.to_string()))
}

fn invalid(m: &str) -> PwcError {
    err(ErrorCode::InvalidArgument, m)
}

fn too_large(m: &str) -> PwcError {
    err(ErrorCode::PayloadTooLarge, m)
}

fn unique<T: PartialEq>(items: &[T]) -> bool {
    items.iter().enumerate().all(|(i, a)| !items[..i].contains(a))
}

/// Check 4: parameter syntax, then the size limits of Core §4.1.
pub fn parse(spec: &MethodSpec, params: Value, limits: &Limits) -> Result<Request, PwcError> {
    let psbt_limit = |b: &Base64| {
        if b.0.len() as u64 > limits.max_psbt_bytes {
            Err(too_large("PSBT exceeds limits.max_psbt_bytes"))
        } else {
            Ok(())
        }
    };
    let message_limit = |m: &str| {
        if m.len() as u64 > limits.max_message_bytes {
            Err(too_large("Message exceeds limits.max_message_bytes"))
        } else {
            Ok(())
        }
    };
    Ok(match spec.name {
        "pwc.getCapabilities" => de::<Empty>(params).map(|_| Request::GetCapabilities)?,
        "session.getInfo" => de::<Empty>(params).map(|_| Request::GetInfo)?,
        "session.requestPermissions" => {
            let p: RequestPermissions = de(params)?;
            if p.scopes.is_empty() || !unique(&p.scopes) {
                return Err(invalid("scopes must be a nonempty list without duplicates"));
            }
            Request::RequestPermissions(p)
        }
        "session.getPermissions" => de::<Empty>(params).map(|_| Request::GetPermissions)?,
        "session.revokePermissions" => Request::RevokePermissions(de(params)?),
        "wallet.list" => de::<Empty>(params).map(|_| Request::WalletList)?,
        "account.list" => Request::AccountList(de(params)?),
        "account.getBalance" => Request::GetBalance(de(params)?),
        "address.getReceiveAddress" => Request::GetReceiveAddress(de(params)?),
        "address.create" => Request::AddressCreate(de(params)?),
        "address.list" => Request::AddressList(de(params)?),
        "utxo.list" => Request::UtxoList(de(params)?),
        "transaction.list" => Request::TransactionList(de(params)?),
        "transaction.get" => Request::TransactionGet(de(params)?),
        "psbt.create" => {
            let p: PsbtCreate = de(params)?;
            if p.outputs.is_empty() {
                return Err(invalid("outputs must not be empty"));
            }
            Request::PsbtCreate(p)
        }
        "psbt.releaseReservation" => Request::ReleaseReservation(de(params)?),
        "psbt.sign" => {
            let p: PsbtSign = de(params)?;
            if p.input_indexes.as_ref().is_some_and(|i| i.is_empty() || !unique(i)) {
                return Err(invalid("input_indexes must be nonempty and unique"));
            }
            psbt_limit(&p.psbt)?;
            Request::PsbtSign(p)
        }
        "psbt.combine" => {
            let p: PsbtCombine = de(params)?;
            if p.psbts.len() < 2 {
                return Err(invalid("psbt.combine needs at least two PSBTs"));
            }
            if p.psbts.len() as u64 > limits.max_combine_psbts {
                return Err(too_large("More PSBTs than limits.max_combine_psbts"));
            }
            p.psbts.iter().try_for_each(psbt_limit)?;
            Request::PsbtCombine(p)
        }
        "psbt.finalize" => {
            let p: PsbtFinalize = de(params)?;
            psbt_limit(&p.psbt)?;
            Request::PsbtFinalize(p)
        }
        "transaction.broadcast" => {
            let p: Broadcast = de(params)?;
            if p.raw_transaction.0.len() as u64 > limits.max_raw_transaction_bytes {
                return Err(too_large("Transaction exceeds limits.max_raw_transaction_bytes"));
            }
            Request::Broadcast(p)
        }
        "message.sign" => {
            let p: MessageSign = de(params)?;
            message_limit(&p.message)?;
            Request::MessageSign(p)
        }
        "message.verify" => {
            let p: MessageVerify = de(params)?;
            message_limit(&p.message)?;
            Request::MessageVerify(p)
        }
        "operation.get" => Request::OperationGet(de(params)?),
        "operation.cancel" => Request::OperationCancel(de(params)?),
        _ => unreachable!("every registered method is parsed"),
    })
}
