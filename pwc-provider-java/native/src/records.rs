//! Backend data types: the Rust records of `pwc_provider::backend` and their Java records.

use jni::objects::{JObject, JValue};
use jni::JNIEnv;
use pwc_provider::backend::{
    AccountInfo, AddressRecord, Balance, CreateRequest, CreatedPsbt, SyncStatus, TransactionRecord, Utxo, WalletInfo,
    WalletKind, WalletState,
};
use pwc_provider::balance::Components;
use pwc_provider::types::{Hex, Sats, SignedSats};

use crate::convert::*;

/// The values of the `&'static str` fields, as in the strict schema.
pub const SYNC_STATES: [&str; 4] = ["synced", "syncing", "offline", "unknown"];
pub const ADDRESS_ROLES: [&str; 3] = ["receive", "change", "unknown"];
pub const ADDRESS_STATES: [&str; 3] = ["unused", "used", "unknown"];
pub const UTXO_RESTRICTIONS: [&str; 5] = ["frozen", "locked", "immature", "unconfirmed", "unknown"];
pub const TRANSACTION_STATUSES: [&str; 6] = ["unconfirmed", "confirmed", "conflicted", "replaced", "dropped", "unknown"];

fn ids<'l, T: AsRef<str>>(env: &mut JNIEnv<'l>, items: &[T]) -> Result<JObject<'l>> {
    strings(env, items)
}

fn sig(fields: &str) -> String {
    format!("({fields})V")
}

fn t(name: &str) -> String {
    format!("L{PKG}{name};")
}

// ---------------------------------------------------------------------------------------------------------
// Rust to Java

pub fn wallet_info<'l>(env: &mut JNIEnv<'l>, w: &WalletInfo) -> Result<JObject<'l>> {
    let id = string(env, w.wallet_id.as_str())?;
    let name = string(env, w.display_name.as_str())?;
    let network = string(env, &w.network)?;
    let kind = serde_enum(env, "WalletKind", &w.kind)?;
    let state = serde_enum(env, "WalletState", &w.state)?;
    let accounts = ids(env, &w.account_ids.iter().map(|a| a.as_str()).collect::<Vec<_>>())?;
    new(
        env,
        "WalletInfo",
        &sig(&format!("{STRING}{STRING}{STRING}{}{}{LIST}", t("WalletKind"), t("WalletState"))),
        &[
            (&id).into(),
            (&name).into(),
            (&network).into(),
            (&kind).into(),
            (&state).into(),
            (&accounts).into(),
        ],
    )
}

pub fn account_info<'l>(env: &mut JNIEnv<'l>, a: &AccountInfo) -> Result<JObject<'l>> {
    let id = string(env, a.account_id.as_str())?;
    let wallet = string(env, a.wallet_id.as_str())?;
    let name = string(env, a.display_name.as_str())?;
    let network = string(env, &a.network)?;
    let script_type = serde_enum(env, "ScriptType", &a.script_type)?;
    let methods = strings(env, &a.methods)?;
    new(
        env,
        "AccountInfo",
        &sig(&format!("{STRING}{STRING}{STRING}{STRING}Z{}{LIST}", t("ScriptType"))),
        &[
            (&id).into(),
            (&wallet).into(),
            (&name).into(),
            (&network).into(),
            JValue::Bool(a.can_sign.into()),
            (&script_type).into(),
            (&methods).into(),
        ],
    )
}

pub fn balance<'l>(env: &mut JNIEnv<'l>, b: &Balance) -> Result<JObject<'l>> {
    let c = &b.components;
    let mut args = Vec::new();
    for v in [c.confirmed_sats, c.pending_self_sats, c.pending_external_sats, c.pending_outgoing_sats] {
        args.push(JValue::Long(u64_long(v.0)?));
    }
    args.push(JValue::Long(c.unconfirmed_delta_sats.0));
    for v in [c.spendable_sats, c.locked_sats, c.immature_sats] {
        args.push(JValue::Long(u64_long(v.0)?));
    }
    let components = new(env, "BalanceComponents", "(JJJJJJJJ)V", &args)?;
    let state = enum_value(env, "SyncState", b.sync.state)?;
    let tip = integer(env, b.sync.tip_height.map(u32_int).transpose()?)?;
    let sync = new(env, "SyncStatus", &sig(&format!("{}{INTEGER}", t("SyncState"))), &[(&state).into(), (&tip).into()])?;
    let id = string(env, b.account_id.as_str())?;
    let network = string(env, &b.network)?;
    let updated = instant(env, b.updated_at)?;
    new(
        env,
        "Balance",
        &sig(&format!("{STRING}{STRING}{}{INSTANT}{}", t("BalanceComponents"), t("SyncStatus"))),
        &[(&id).into(), (&network).into(), (&components).into(), (&updated).into(), (&sync).into()],
    )
}

pub fn address_record<'l>(env: &mut JNIEnv<'l>, a: &AddressRecord) -> Result<JObject<'l>> {
    let id = string(env, a.account_id.as_str())?;
    let network = string(env, &a.network)?;
    let address = string(env, &a.address)?;
    let role = enum_value(env, "AddressRole", a.role)?;
    let state = enum_value(env, "AddressState", a.state)?;
    let label = opt_string(env, a.label.as_ref().map(|l| l.as_str()))?;
    new(
        env,
        "AddressRecord",
        &sig(&format!("{STRING}{STRING}{STRING}{}{}{STRING}", t("AddressRole"), t("AddressState"))),
        &[
            (&id).into(),
            (&network).into(),
            (&address).into(),
            (&role).into(),
            (&state).into(),
            (&label).into(),
        ],
    )
}

pub fn address_records<'l>(env: &mut JNIEnv<'l>, items: &[AddressRecord]) -> Result<JObject<'l>> {
    let mut out = Vec::new();
    for a in items {
        out.push(address_record(env, a)?);
    }
    list(env, out)
}

pub fn utxo<'l>(env: &mut JNIEnv<'l>, u: &Utxo) -> Result<JObject<'l>> {
    let txid = sha256(env, u.txid.as_str())?;
    let id = string(env, u.account_id.as_str())?;
    let network = string(env, &u.network)?;
    let height = integer(env, u.block_height.map(u32_int).transpose()?)?;
    let address = string(env, &u.address)?;
    let script = script(env, &u.script_pub_key.0)?;
    let mut restrictions = Vec::new();
    for r in &u.restrictions {
        restrictions.push(enum_value(env, "UtxoRestriction", r)?);
    }
    let restrictions = list(env, restrictions)?;
    new(
        env,
        "Utxo",
        &sig(&format!("{SHA256}I{STRING}{STRING}JI{INTEGER}{STRING}{SCRIPT}Z{LIST}Z")),
        &[
            (&txid).into(),
            JValue::Int(u32_int(u.vout)?),
            (&id).into(),
            (&network).into(),
            JValue::Long(u64_long(u.value_sats.0)?),
            JValue::Int(u32_int(u.confirmations)?),
            (&height).into(),
            (&address).into(),
            (&script).into(),
            JValue::Bool(u.spendable.into()),
            (&restrictions).into(),
            JValue::Bool(u.is_change.into()),
        ],
    )
}

pub fn transaction_record<'l>(env: &mut JNIEnv<'l>, r: &TransactionRecord) -> Result<JObject<'l>> {
    let id = string(env, r.account_id.as_str())?;
    let network = string(env, &r.network)?;
    let txid = sha256(env, r.txid.as_str())?;
    let status = enum_value(env, "TransactionStatus", r.status)?;
    let height = integer(env, r.block_height.map(u32_int).transpose()?)?;
    let first = opt_instant(env, r.first_seen_at)?;
    let confirmed = opt_instant(env, r.confirmed_at)?;
    let fee = long(env, r.fee_sats.map(|f| u64_long(f.0)).transpose()?)?;
    let replaced = opt_sha256(env, r.replaced_by_txid.as_ref().map(|h| h.as_str()))?;
    new(
        env,
        "TransactionRecord",
        &sig(&format!(
            "{STRING}{STRING}{SHA256}{}I{INTEGER}{INSTANT}{INSTANT}J{LONG}{SHA256}",
            t("TransactionStatus")
        )),
        &[
            (&id).into(),
            (&network).into(),
            (&txid).into(),
            (&status).into(),
            JValue::Int(u32_int(r.confirmations)?),
            (&height).into(),
            (&first).into(),
            (&confirmed).into(),
            JValue::Long(r.net_change_sats.0),
            (&fee).into(),
            (&replaced).into(),
        ],
    )
}

pub fn create_request<'l>(env: &mut JNIEnv<'l>, r: &CreateRequest) -> Result<JObject<'l>> {
    let mut outputs = Vec::new();
    for (script, value) in &r.outputs {
        outputs.push(txout(env, *value, script.as_bytes())?);
    }
    let outputs = list(env, outputs)?;
    let selected = match &r.selected {
        Some(s) => outpoints(env, s.iter().copied())?,
        None => JObject::null(),
    };
    new(
        env,
        "CreateRequest",
        &sig(&format!("{LIST}J{LIST}Z")),
        &[
            (&outputs).into(),
            JValue::Long(u64_long(r.fee_rate)?),
            (&selected).into(),
            JValue::Bool(r.replaceable.into()),
        ],
    )
}

pub fn created_psbt<'l>(env: &mut JNIEnv<'l>, c: &CreatedPsbt) -> Result<JObject<'l>> {
    let psbt = bytes(env, &c.psbt.serialize())?;
    let mut change = Vec::new();
    for i in &c.change_output_indexes {
        change.push(u32_int(*i)?);
    }
    let change = ints(env, change)?;
    let inputs = outpoints(env, c.inputs.iter().copied())?;
    new(
        env,
        "CreatedPsbt",
        &sig(&format!("[BJJ{LIST}{LIST}")),
        &[
            (&psbt).into(),
            JValue::Long(u64_long(c.fee)?),
            JValue::Long(u64_long(c.vsize)?),
            (&change).into(),
            (&inputs).into(),
        ],
    )
}

pub fn signed_psbt<'l>(env: &mut JNIEnv<'l>, psbt: &bitcoin::Psbt, signed: &[u32]) -> Result<JObject<'l>> {
    let b = bytes(env, &psbt.serialize())?;
    let mut indexes = Vec::new();
    for i in signed {
        indexes.push(u32_int(*i)?);
    }
    let indexes = ints(env, indexes)?;
    new(env, "SignedPsbt", &sig(&format!("[B{LIST}")), &[(&b).into(), (&indexes).into()])
}

// ---------------------------------------------------------------------------------------------------------
// Java to Rust

pub fn wallet_kind(env: &mut JNIEnv, e: &JObject) -> Result<WalletKind> {
    Ok(match enum_name(env, e, '_')?.as_str() {
        "software" => WalletKind::Software,
        "hardware" => WalletKind::Hardware,
        "multisig" => WalletKind::Multisig,
        "watch_only" => WalletKind::WatchOnly,
        _ => WalletKind::Other,
    })
}

pub fn wallet_state(env: &mut JNIEnv, e: &JObject) -> Result<WalletState> {
    Ok(match enum_name(env, e, '_')?.as_str() {
        "ready" => WalletState::Ready,
        "locked" => WalletState::Locked,
        "syncing" => WalletState::Syncing,
        _ => WalletState::Unavailable,
    })
}

pub fn from_wallet_info(env: &mut JNIEnv, w: &JObject) -> Result<WalletInfo> {
    let kind = obj(env, w, "kind", &t("WalletKind"))?;
    let state = obj(env, w, "state", &t("WalletState"))?;
    Ok(WalletInfo {
        wallet_id: parse(str_field(env, w, "walletId")?)?,
        display_name: parse(str_field(env, w, "displayName")?)?,
        network: parse::<pwc_provider::types::Network>(str_field(env, w, "network")?)?.0,
        kind: wallet_kind(env, &kind)?,
        state: wallet_state(env, &state)?,
        account_ids: parse_all(string_list_field(env, w, "accountIds")?)?,
    })
}

pub fn from_account_info(env: &mut JNIEnv, a: &JObject) -> Result<AccountInfo> {
    let script_type = obj(env, a, "scriptType", &t("ScriptType"))?;
    let methods = string_list_field(env, a, "methods")?
        .iter()
        .filter_map(|m| pwc_provider::methods::METHODS.iter().find(|s| s.name == m).map(|s| s.name))
        .collect();
    Ok(AccountInfo {
        account_id: parse(str_field(env, a, "accountId")?)?,
        wallet_id: parse(str_field(env, a, "walletId")?)?,
        display_name: parse(str_field(env, a, "displayName")?)?,
        network: parse::<pwc_provider::types::Network>(str_field(env, a, "network")?)?.0,
        can_sign: bool_field(env, a, "canSign")?,
        script_type: from_serde_enum(env, &script_type, '_')?,
        methods,
    })
}

fn sats(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Sats> {
    u64_field(env, o, name).map(Sats)
}

pub fn from_balance(env: &mut JNIEnv, b: &JObject) -> Result<Balance> {
    let c = obj(env, b, "components", &t("BalanceComponents"))?;
    let components = Components {
        confirmed_sats: sats(env, &c, "confirmedSats")?,
        pending_self_sats: sats(env, &c, "pendingSelfSats")?,
        pending_external_sats: sats(env, &c, "pendingExternalSats")?,
        pending_outgoing_sats: sats(env, &c, "pendingOutgoingSats")?,
        unconfirmed_delta_sats: SignedSats(long_field(env, &c, "unconfirmedDeltaSats")?),
        spendable_sats: sats(env, &c, "spendableSats")?,
        locked_sats: sats(env, &c, "lockedSats")?,
        immature_sats: sats(env, &c, "immatureSats")?,
    };
    let s = obj(env, b, "sync", &t("SyncStatus"))?;
    let state = enum_field(env, &s, "state", "SyncState", '_')?;
    Ok(Balance {
        account_id: parse(str_field(env, b, "accountId")?)?,
        network: parse::<pwc_provider::types::Network>(str_field(env, b, "network")?)?.0,
        components,
        updated_at: instant_field(env, b, "updatedAt")?,
        sync: SyncStatus {
            state: static_str(&state, &SYNC_STATES)?,
            tip_height: opt_u32_field(env, &s, "tipHeight")?,
        },
    })
}

pub fn from_address_record(env: &mut JNIEnv, a: &JObject) -> Result<AddressRecord> {
    let role = enum_field(env, a, "role", "AddressRole", '_')?;
    let state = enum_field(env, a, "state", "AddressState", '_')?;
    Ok(AddressRecord {
        account_id: parse(str_field(env, a, "accountId")?)?,
        network: parse::<pwc_provider::types::Network>(str_field(env, a, "network")?)?.0,
        address: parse::<pwc_provider::types::Address>(str_field(env, a, "address")?)?.0,
        role: static_str(&role, &ADDRESS_ROLES)?,
        state: static_str(&state, &ADDRESS_STATES)?,
        label: opt_str_field(env, a, "label")?.map(parse).transpose()?,
    })
}

pub fn from_utxo(env: &mut JNIEnv, u: &JObject) -> Result<Utxo> {
    let txid = obj(env, u, "txid", SHA256)?;
    let script = obj(env, u, "scriptPubKey", SCRIPT)?;
    let mut restrictions = Vec::new();
    for r in list_field(env, u, "restrictions")? {
        let name = enum_name(env, &r, '_')?;
        restrictions.push(static_str(&name, &UTXO_RESTRICTIONS)?);
    }
    Ok(Utxo {
        txid: parse(from_sha256(env, &txid)?)?,
        vout: u32_field(env, u, "vout")?,
        account_id: parse(str_field(env, u, "accountId")?)?,
        network: parse::<pwc_provider::types::Network>(str_field(env, u, "network")?)?.0,
        value_sats: sats(env, u, "valueSats")?,
        confirmations: u32_field(env, u, "confirmations")?,
        block_height: opt_u32_field(env, u, "blockHeight")?,
        address: parse::<pwc_provider::types::Address>(str_field(env, u, "address")?)?.0,
        script_pub_key: Hex(from_script(env, &script)?.into_bytes()),
        spendable: bool_field(env, u, "spendable")?,
        restrictions,
        is_change: bool_field(env, u, "isChange")?,
    })
}

pub fn from_transaction_record(env: &mut JNIEnv, r: &JObject) -> Result<TransactionRecord> {
    let txid = obj(env, r, "txid", SHA256)?;
    let status = enum_field(env, r, "status", "TransactionStatus", '_')?;
    let replaced = obj(env, r, "replacedByTxid", SHA256)?;
    let replaced = if replaced.is_null() {
        None
    } else {
        Some(parse(from_sha256(env, &replaced)?)?)
    };
    Ok(TransactionRecord {
        account_id: parse(str_field(env, r, "accountId")?)?,
        network: parse::<pwc_provider::types::Network>(str_field(env, r, "network")?)?.0,
        txid: parse(from_sha256(env, &txid)?)?,
        status: static_str(&status, &TRANSACTION_STATUSES)?,
        confirmations: u32_field(env, r, "confirmations")?,
        block_height: opt_u32_field(env, r, "blockHeight")?,
        first_seen_at: opt_instant_field(env, r, "firstSeenAt")?,
        confirmed_at: opt_instant_field(env, r, "confirmedAt")?,
        net_change_sats: SignedSats(long_field(env, r, "netChangeSats")?),
        fee_sats: opt_u64_field(env, r, "feeSats")?.map(Sats),
        replaced_by_txid: replaced,
    })
}

pub fn from_create_request(env: &mut JNIEnv, r: &JObject) -> Result<CreateRequest> {
    let mut outputs = Vec::new();
    for o in list_field(env, r, "outputs")? {
        let out = from_txout(env, &o)?;
        outputs.push((out.script_pubkey, out.value.to_sat()));
    }
    let selected = obj(env, r, "selected", LIST)?;
    let selected = if selected.is_null() {
        None
    } else {
        Some(outpoint_list(env, &selected)?)
    };
    Ok(CreateRequest {
        outputs,
        fee_rate: u64_field(env, r, "feeRate")?,
        selected,
        replaceable: bool_field(env, r, "replaceable")?,
    })
}

pub fn from_created_psbt(env: &mut JNIEnv, c: &JObject) -> Result<CreatedPsbt> {
    let psbt = bytes_field(env, c, "psbt")?;
    let change = obj(env, c, "changeOutputIndexes", LIST)?;
    let inputs = obj(env, c, "inputs", LIST)?;
    Ok(CreatedPsbt {
        psbt: bitcoin::Psbt::deserialize(&psbt).map_err(|e| invalid(e))?,
        fee: u64_field(env, c, "fee")?,
        vsize: u64_field(env, c, "vsize")?,
        change_output_indexes: int_list(env, &change)?,
        inputs: outpoint_list(env, &inputs)?,
    })
}

/// A `SignedPsbt` as the PSBT and the indexes of the inputs that gained a signature.
pub fn from_signed_psbt(env: &mut JNIEnv, s: &JObject) -> Result<(bitcoin::Psbt, Vec<u32>)> {
    let psbt = bytes_field(env, s, "psbt")?;
    let signed = obj(env, s, "signedInputs", LIST)?;
    Ok((
        bitcoin::Psbt::deserialize(&psbt).map_err(|e| invalid(e))?,
        int_list(env, &signed)?,
    ))
}
