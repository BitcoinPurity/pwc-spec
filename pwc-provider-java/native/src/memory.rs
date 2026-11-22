//! Native methods of `MemoryBackend`.

use std::sync::Arc;

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jint, jlong, jobject, jstring, JNI_FALSE};
use jni::JNIEnv;
use pwc_provider::backend::WalletBackend;
use pwc_provider::error::PwcError;
use pwc_provider::memory::MemoryBackend;
use pwc_provider::types::{Id, MessageFormat, ScriptType, Text};

use crate::convert::*;
use crate::native;
use crate::provider::{free, get, into_ptr, MemoryHandle};
use crate::records::*;

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryDemo<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
) -> jlong {
    native(&mut env, 0, |_| {
        Ok(into_ptr::<MemoryHandle>(Arc::new(MemoryBackend::demo())))
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryFree<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
) {
    // SAFETY: called once by the Java handle's cleaner.
    native(&mut env, (), |_| {
        unsafe { free::<MemoryHandle>(m) };
        Ok(())
    })
}

/// # Safety
/// `m` is a live `MemoryBackend` handle.
unsafe fn mem<'a>(m: jlong) -> &'a MemoryBackend {
    get::<MemoryHandle>(m)
}

fn count(v: jint) -> Result<u32> {
    u32::try_from(v).map_err(|_| invalid("a count is negative"))
}

fn amount(v: jlong) -> Result<u64> {
    u64::try_from(v).map_err(|_| invalid("an amount is negative"))
}

fn id(env: &mut JNIEnv, s: &JString) -> Result<Id> {
    parse(get_string(env, s)?)
}

fn text(env: &mut JNIEnv, s: &JString) -> Result<String> {
    get_string(env, s)
}

/// Runs `f` on the backend of handle `m`; the Java `MemoryBackend` keeps the handle alive during the call.
fn with<'l, T>(
    env: &mut JNIEnv<'l>,
    m: jlong,
    default: T,
    f: impl FnOnce(&mut JNIEnv<'l>, &MemoryBackend) -> Result<T>,
) -> T {
    // SAFETY: see above.
    native(env, default, |env| f(env, unsafe { mem(m) }))
}

const NULL: jobject = std::ptr::null_mut();

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryAddWallet<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    wallet_id: JString<'l>,
    name: JString<'l>,
    network: JString<'l>,
    kind: JObject<'l>,
) -> jstring {
    with(&mut env, m, NULL, |env, b| {
        let wallet = id(env, &wallet_id)?;
        let name: Text = parse(text(env, &name)?)?;
        let network = text(env, &network)?;
        let kind = wallet_kind(env, &kind)?;
        let id = b.add_wallet(wallet.as_str(), name.as_str(), &network, kind);
        Ok(string(env, id.as_str())?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryAddAccount<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    wallet_id: JString<'l>,
    account_id: JString<'l>,
    name: JString<'l>,
    script_type: JObject<'l>,
    can_sign: jboolean,
) -> jstring {
    with(&mut env, m, NULL, |env, b| {
        let wallet = id(env, &wallet_id)?;
        let account = id(env, &account_id)?;
        let name: Text = parse(text(env, &name)?)?;
        let script_type: ScriptType = from_serde_enum(env, &script_type, '_')?;
        let id = b.add_account(
            &wallet,
            account.as_str(),
            name.as_str(),
            script_type,
            can_sign != JNI_FALSE,
        );
        Ok(string(env, id.as_str())?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memorySetWalletState<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    wallet_id: JString<'l>,
    state: JObject<'l>,
) {
    with(&mut env, m, (), |env, b| {
        let wallet = id(env, &wallet_id)?;
        let state = wallet_state(env, &state)?;
        b.set_wallet_state(wallet.as_str(), state);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryReceive<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    value: jlong,
    confirmations: jint,
    coinbase: jboolean,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = id(env, &account_id)?;
        let (value, confirmations) = (amount(value)?, count(confirmations)?);
        let txid = if coinbase != JNI_FALSE {
            b.receive_coinbase(&account, value, confirmations)
        } else {
            b.receive(&account, value, confirmations)
        };
        Ok(sha256(env, &txid.to_string())?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryReceiveConflicting<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    txid_of: JObject<'l>,
    value: jlong,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = id(env, &account_id)?;
        let conflicting = txid(env, &txid_of)?;
        let new = b.receive_conflicting(&account, &conflicting, amount(value)?);
        Ok(sha256(env, &new.to_string())?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryReplace<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    replaced: JObject<'l>,
    by: JObject<'l>,
) {
    with(&mut env, m, (), |env, b| {
        let network = text(env, &network)?;
        let (replaced, by) = (txid(env, &replaced)?, txid(env, &by)?);
        b.replace(&network, &replaced, &by);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryDropPending<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    dropped: JObject<'l>,
) {
    with(&mut env, m, (), |env, b| {
        let network = text(env, &network)?;
        let dropped = txid(env, &dropped)?;
        b.drop_pending(&network, &dropped);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryMine<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    blocks: jint,
) {
    with(&mut env, m, (), |env, b| {
        let network = text(env, &network)?;
        b.mine(&network, count(blocks)?);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryReorg<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    depth: jint,
) {
    with(&mut env, m, (), |env, b| {
        let network = text(env, &network)?;
        b.reorg(&network, count(depth)?);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memorySetOffline<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    offline: jboolean,
) {
    with(&mut env, m, (), |env, b| {
        let network = text(env, &network)?;
        b.set_offline(&network, offline != JNI_FALSE);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memorySetFeeEstimate<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    present: jboolean,
    sat_per_kvb: jlong,
) {
    with(&mut env, m, (), |env, b| {
        let network = text(env, &network)?;
        let rate = if present != JNI_FALSE {
            Some(amount(sat_per_kvb)?)
        } else {
            None
        };
        b.set_fee_estimate(&network, rate);
        Ok(())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memorySpendByUser<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    outpoint: JObject<'l>,
    value: jlong,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = id(env, &account_id)?;
        let outpoint = from_outpoint(env, &outpoint)?;
        let txid = b.spend_by_user(&account, &outpoint, amount(value)?);
        Ok(sha256(env, &txid.to_string())?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryTip<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
) -> jint {
    with(&mut env, m, 0, |env, b| {
        let network = text(env, &network)?;
        u32_int(b.tip(&network))
    })
}

// ---------------------------------------------------------------------------------------------------------
// WalletBackend

fn pwc<T>(r: std::result::Result<T, PwcError>) -> Result<T> {
    r.map_err(Error::Pwc)
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryWallets<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let mut out = Vec::new();
        for w in b.wallets() {
            out.push(wallet_info(env, &w)?);
        }
        Ok(list(env, out)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryAccount<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        match b.account(&account) {
            Some(a) => Ok(account_info(env, &a)?.into_raw()),
            None => Ok(NULL),
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryBalance<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    reserved: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let reserved = from_outpoint_set(env, &reserved)?;
        let r = pwc(b.balance(&account, &reserved))?;
        Ok(balance(env, &r)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryReceiveAddress<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let r = pwc(b.receive_address(&account))?;
        Ok(address_record(env, &r)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryCreateAddress<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    label: JString<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let label: Option<Text> = get_opt_string(env, &label)?.map(parse).transpose()?;
        let r = pwc(b.create_address(&account, label.as_ref()))?;
        Ok(address_record(env, &r)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryAddresses<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        Ok(address_records(env, &b.addresses(&account))?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryUtxos<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    reserved: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let reserved = from_outpoint_set(env, &reserved)?;
        let mut out = Vec::new();
        for u in pwc(b.utxos(&account, &reserved))? {
            out.push(utxo(env, &u)?);
        }
        Ok(list(env, out)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryTransactions<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let mut out = Vec::new();
        for r in pwc(b.transactions(&account))? {
            out.push(transaction_record(env, &r)?);
        }
        Ok(list(env, out)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryOwnsScript<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    script: JObject<'l>,
) -> jboolean {
    with(&mut env, m, JNI_FALSE, |env, b| {
        let account = text(env, &account_id)?;
        let script = from_script(env, &script)?;
        Ok(b.owns_script(&account, &script).into())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryIsUnspent<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    outpoint: JObject<'l>,
) -> jboolean {
    with(&mut env, m, JNI_FALSE, |env, b| {
        let account = text(env, &account_id)?;
        let outpoint = from_outpoint(env, &outpoint)?;
        Ok(b.is_unspent(&account, &outpoint).into())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryFeeEstimate<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    target_blocks: jlong,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let network = text(env, &network)?;
        let estimate = b.fee_estimate(&network, amount(target_blocks)?);
        Ok(long(env, estimate.map(u64_long).transpose()?)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryCreatePsbt<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    request: JObject<'l>,
    excluded: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let request = from_create_request(env, &request)?;
        let excluded = from_outpoint_set(env, &excluded)?;
        let created = pwc(b.create_psbt(&account, &request, &excluded))?;
        Ok(created_psbt(env, &created)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryOwnedOutput<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    outpoint: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let outpoint = from_outpoint(env, &outpoint)?;
        match b.owned_output(&account, &outpoint) {
            Some(o) => Ok(txout(env, o.value.to_sat(), o.script_pubkey.as_bytes())?.into_raw()),
            None => Ok(NULL),
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memorySignPsbt<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    psbt: JObject<'l>,
    inputs: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let bytes = get_bytes(env, &psbt)?;
        let mut psbt = bitcoin::Psbt::deserialize(&bytes).map_err(invalid)?;
        let inputs: Vec<usize> = int_list(env, &inputs)?.into_iter().map(|i| i as usize).collect();
        if inputs.iter().any(|&i| i >= psbt.inputs.len()) {
            return Err(invalid("an input index is out of range"));
        }
        let signed = pwc(b.sign_psbt(&account, &mut psbt, &inputs))?;
        Ok(signed_psbt(env, &psbt, &signed)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memorySignMessage<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    account_id: JString<'l>,
    script: JObject<'l>,
    message: JString<'l>,
    format: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let account = text(env, &account_id)?;
        let script = from_script(env, &script)?;
        let message = text(env, &message)?;
        let format: MessageFormat = from_serde_enum(env, &format, '-')?;
        let signature = pwc(b.sign_message(&account, &script, &message, format))?;
        Ok(bytes(env, &signature)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryBroadcast<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
    network: JString<'l>,
    tx: JObject<'l>,
) -> jobject {
    with(&mut env, m, NULL, |env, b| {
        let network = text(env, &network)?;
        let tx = from_transaction(env, &tx)?;
        let txid = pwc(b.broadcast(&network, &tx))?;
        Ok(sha256(env, &txid.to_string())?.into_raw())
    })
}
