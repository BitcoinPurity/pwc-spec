//! Native methods of `Provider`, `LocalServer` and `Prompt`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jbyteArray, jlong, jobject, jstring, JNI_FALSE};
use jni::JNIEnv;
use pwc_provider::backend::{BackendEvent, WalletBackend};
use pwc_provider::local::{self, EndpointError};
use pwc_provider::memory::MemoryBackend;
use pwc_provider::provider::{PairedClient, Provider, StateError};
use pwc_provider::types::{Id, Timestamp};
use serde_json::Value;
use tokio::task::JoinHandle;

use crate::convert::*;
use crate::records::TRANSACTION_STATUSES;
use crate::{config_from_java, native, prompts};

pub type ProviderHandle = Arc<Provider>;
pub type MemoryHandle = Arc<MemoryBackend>;

pub struct ServerHandle {
    task: JoinHandle<()>,
    path: PathBuf,
}

pub fn into_ptr<T>(v: T) -> jlong {
    Box::into_raw(Box::new(v)) as jlong
}

/// # Safety
/// `ptr` comes from `into_ptr::<T>` and has not been freed.
pub unsafe fn get<'a, T>(ptr: jlong) -> &'a T {
    &*(ptr as *const T)
}

/// # Safety
/// `ptr` comes from `into_ptr::<T>` and is freed only here, once.
pub unsafe fn free<T>(ptr: jlong) {
    drop(Box::from_raw(ptr as *mut T));
}

fn backend(env: &mut JNIEnv, memory: jlong, java: &JObject) -> Result<Arc<dyn WalletBackend>> {
    if memory != 0 {
        // SAFETY: the Java caller passes the live handle of a MemoryBackend it keeps reachable.
        return Ok(unsafe { get::<MemoryHandle>(memory) }.clone());
    }
    let _ = (env, java);
    Err(invalid("Java backends are not implemented yet"))
}

fn open(
    env: &mut JNIEnv,
    memory: jlong,
    java: &JObject,
    config: &JObject,
    dispatcher: &JObject,
    snapshot: Option<&JObject>,
) -> Result<jlong> {
    let backend = backend(env, memory, java)?;
    let config = config_from_java(env, config)?;
    let dispatcher = env.new_global_ref(dispatcher)?;
    let (provider, prompts) = match snapshot {
        None => Provider::new(backend, config),
        Some(s) => {
            let bytes = get_bytes(env, s)?;
            Provider::with_state(backend, config, &bytes).map_err(|e| {
                let kind = match e {
                    StateError::Corrupt(_) => "CORRUPT",
                    StateError::UnsupportedVersion(_) => "UNSUPPORTED_VERSION",
                };
                Error::Kind {
                    class: "org/bitcoinpurity/pwc/provider/StateException",
                    kind,
                    message: e.to_string(),
                    path: None,
                }
            })?
        }
    };
    prompts::spawn(prompts, dispatcher);
    Ok(into_ptr::<ProviderHandle>(provider))
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerCreate<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    memory: jlong,
    backend: JObject<'l>,
    config: JObject<'l>,
    dispatcher: JObject<'l>,
) -> jlong {
    native(&mut env, 0, |env| open(env, memory, &backend, &config, &dispatcher, None))
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerRestore<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    memory: jlong,
    backend: JObject<'l>,
    config: JObject<'l>,
    dispatcher: JObject<'l>,
    snapshot: JObject<'l>,
) -> jlong {
    native(&mut env, 0, |env| {
        open(env, memory, &backend, &config, &dispatcher, Some(&snapshot))
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerFree<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    provider: jlong,
) {
    // SAFETY: called once by the Java handle's cleaner.
    native(&mut env, (), |_| Ok(unsafe { free::<ProviderHandle>(provider) }))
}

/// # Safety
/// `ptr` is a live Provider handle.
unsafe fn provider<'a>(ptr: jlong) -> &'a Provider {
    get::<ProviderHandle>(ptr)
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerSaveState<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
) -> jbyteArray {
    native(&mut env, std::ptr::null_mut(), |env| {
        // SAFETY: the Java Provider keeps its handle alive during the call.
        let state = unsafe { provider(p) }.save_state();
        Ok(bytes(env, &state)?.into_raw())
    })
}

fn paired_client<'l>(env: &mut JNIEnv<'l>, c: &PairedClient) -> Result<JObject<'l>> {
    let id = string(env, c.client_id.as_str())?;
    let name = string(env, c.claimed_name.as_str())?;
    let description = opt_string(env, c.claimed_description.as_ref().map(|d| d.as_str()))?;
    let paired = instant(env, c.paired_at)?;
    let last = instant(env, c.last_connected_at)?;
    let peer = prompts::peer(env, &c.peer)?;
    new(
        env,
        "PairedClient",
        &format!("({STRING}{STRING}{STRING}{INSTANT}{INSTANT}L{PKG}PeerInfo;)V"),
        &[
            (&id).into(),
            (&name).into(),
            (&description).into(),
            (&paired).into(),
            (&last).into(),
            (&peer).into(),
        ],
    )
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerPairedClients<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
) -> jobject {
    native(&mut env, std::ptr::null_mut(), |env| {
        let mut out = Vec::new();
        // SAFETY: the Java Provider keeps its handle alive during the call.
        for c in unsafe { provider(p) }.paired_clients() {
            out.push(paired_client(env, &c)?);
        }
        Ok(list(env, out)?.into_raw())
    })
}

fn json_timestamp(v: &Value) -> Result<Option<Timestamp>> {
    match v {
        Value::Null => Ok(None),
        v => serde_json::from_value(v.clone()).map(Some).map_err(|e| invalid(e)),
    }
}

fn json_strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// A `Grant` JSON object of `Provider::grants` as a Java `Grant`.
fn grant<'l>(env: &mut JNIEnv<'l>, g: &Value) -> Result<JObject<'l>> {
    let text = |k: &str| g[k].as_str().unwrap_or_default().to_string();
    let grant_id = string(env, &text("grant_id"))?;
    let client_id = string(env, &text("client_id"))?;
    let network = string(env, &text("network"))?;
    let scopes = strings(env, &json_strings(&g["scopes"]))?;
    let wallets = strings(env, &json_strings(&g["wallet_ids"]))?;
    let accounts = strings(env, &json_strings(&g["account_ids"]))?;
    let created = opt_instant(env, json_timestamp(&g["created_at"])?)?;
    let expires = opt_instant(env, json_timestamp(&g["expires_at"])?)?;
    let status = enum_value(env, "GrantStatus", &text("status"))?;
    let constraints = match g.get("constraints") {
        Some(c) => {
            let c = serde_json::from_value(c.clone()).map_err(|e| invalid(e))?;
            prompts::constraints(env, Some(&c))?
        }
        None => JObject::null(),
    };
    new(
        env,
        "Grant",
        &format!(
            "({STRING}{STRING}{STRING}{LIST}{LIST}{LIST}{INSTANT}{INSTANT}L{PKG}GrantStatus;L{PKG}GrantConstraints;)V"
        ),
        &[
            (&grant_id).into(),
            (&client_id).into(),
            (&network).into(),
            (&scopes).into(),
            (&wallets).into(),
            (&accounts).into(),
            (&created).into(),
            (&expires).into(),
            (&status).into(),
            (&constraints).into(),
        ],
    )
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerGrants<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
) -> jobject {
    native(&mut env, std::ptr::null_mut(), |env| {
        let mut out = Vec::new();
        // SAFETY: the Java Provider keeps its handle alive during the call.
        for g in unsafe { provider(p) }.grants() {
            out.push(grant(env, &g)?);
        }
        Ok(list(env, out)?.into_raw())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerRevokeGrant<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
    grant_id: JString<'l>,
) -> jboolean {
    native(&mut env, JNI_FALSE, |env| {
        let id = get_string(env, &grant_id)?;
        // SAFETY: the Java Provider keeps its handle alive during the call.
        Ok(unsafe { provider(p) }.revoke_grant(&id).into())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerUnpairClient<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
    client_id: JString<'l>,
) {
    native(&mut env, (), |env| {
        let id = get_string(env, &client_id)?;
        // SAFETY: the Java Provider keeps its handle alive during the call.
        unsafe { provider(p) }.unpair_client(&id);
        Ok(())
    })
}

fn event_from_java(env: &mut JNIEnv, e: &JObject) -> Result<BackendEvent> {
    let kind = |env: &mut JNIEnv, name: &str| -> Result<bool> {
        let c = sdk(env, &format!("BackendEvent${name}"))?;
        Ok(env.is_instance_of(e, &c)?)
    };
    let id = |env: &mut JNIEnv, field: &str| -> Result<Id> { parse(str_field(env, e, field)?) };
    if kind(env, "WalletLocked")? {
        Ok(BackendEvent::WalletLocked {
            wallet_id: id(env, "walletId")?,
        })
    } else if kind(env, "WalletUnlocked")? {
        Ok(BackendEvent::WalletUnlocked {
            wallet_id: id(env, "walletId")?,
        })
    } else if kind(env, "WalletSyncChanged")? {
        Ok(BackendEvent::WalletSyncChanged {
            wallet_id: id(env, "walletId")?,
        })
    } else if kind(env, "BalanceChanged")? {
        Ok(BackendEvent::BalanceChanged {
            account_id: id(env, "accountId")?,
        })
    } else {
        let txid = obj(env, e, "txid", SHA256)?;
        let status = enum_field(env, e, "status", "TransactionStatus", '_')?;
        Ok(BackendEvent::TransactionChanged {
            account_id: id(env, "accountId")?,
            txid: from_sha256(env, &txid)?,
            status: static_str(&status, &TRANSACTION_STATUSES)?,
        })
    }
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_providerNotify<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
    event: JObject<'l>,
) {
    native(&mut env, (), |env| {
        let event = event_from_java(env, &event)?;
        // SAFETY: the Java Provider keeps its handle alive during the call.
        unsafe { provider(p) }.notify(event);
        Ok(())
    })
}

// ---------------------------------------------------------------------------------------------------------
// LOCAL server

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_serverDefaultDirectory<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
) -> jstring {
    native(&mut env, std::ptr::null_mut(), |env| {
        let dir = local::default_directory();
        Ok(opt_string(env, dir.as_ref().map(|d| d.to_string_lossy()).as_deref())?.into_raw())
    })
}

fn endpoint_error(e: EndpointError) -> Error {
    let (kind, path) = match &e {
        EndpointError::UnsafeDirectory(p) => ("UNSAFE_DIRECTORY", p),
        EndpointError::PathTooLong(p) => ("PATH_TOO_LONG", p),
        EndpointError::AlreadyRunning(p) => ("ALREADY_RUNNING", p),
        EndpointError::Io(p, _) => ("IO", p),
    };
    Error::Kind {
        class: "org/bitcoinpurity/pwc/provider/EndpointException",
        kind,
        path: Some(path.to_string_lossy().into_owned()),
        message: e.to_string(),
    }
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_serverStart<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    p: jlong,
    dir: JString<'l>,
    provider_id: JString<'l>,
) -> jlong {
    native(&mut env, 0, |env| {
        let dir = get_string(env, &dir)?;
        let id = get_string(env, &provider_id)?;
        let endpoint = local::bind(Path::new(&dir), &id).map_err(endpoint_error)?;
        let path = endpoint.path().to_path_buf();
        // SAFETY: the Java Provider keeps its handle alive during the call.
        let provider = unsafe { get::<ProviderHandle>(p) }.clone();
        let task = crate::runtime().spawn(local::serve(provider, endpoint));
        Ok(into_ptr(ServerHandle { task, path }))
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_serverPath<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    s: jlong,
) -> jstring {
    native(&mut env, std::ptr::null_mut(), |env| {
        // SAFETY: called by the LocalServer constructor with the live handle.
        let server = unsafe { get::<ServerHandle>(s) };
        Ok(string(env, &server.path.to_string_lossy())?.into_raw())
    })
}

/// Stops serving; returns once the endpoint is dropped and its socket file removed.
#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_serverClose<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    s: jlong,
) {
    native(&mut env, (), |_| {
        // SAFETY: called once by the Java handle's cleaner.
        let server = unsafe { Box::from_raw(s as *mut ServerHandle) };
        server.task.abort();
        while !server.task.is_finished() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    })
}

// ---------------------------------------------------------------------------------------------------------
// Prompts

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_promptIsWithdrawn<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    h: jlong,
) -> jboolean {
    // SAFETY: the Java Prompt keeps its handle alive during the call.
    native(&mut env, JNI_FALSE, |_| Ok(prompts::is_withdrawn(unsafe { prompts::handle(h) }).into()))
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_promptAnswer<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    h: jlong,
    approve: jboolean,
) -> jboolean {
    native(&mut env, JNI_FALSE, |_| {
        // SAFETY: the Java Prompt keeps its handle alive during the call.
        Ok(prompts::answer(unsafe { prompts::handle(h) }, approve != JNI_FALSE).into())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_promptGrant<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    h: jlong,
    choice: JObject<'l>,
) -> jboolean {
    native(&mut env, JNI_FALSE, |env| {
        let choice = prompts::grant_choice(env, &choice)?;
        // SAFETY: the Java Prompt keeps its handle alive during the call.
        Ok(prompts::grant(unsafe { prompts::handle(h) }, choice)?.into())
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_promptFree<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    h: jlong,
) {
    // SAFETY: called once by the Java handle's cleaner.
    native(&mut env, (), |_| Ok(unsafe { free::<prompts::ReplyHandle>(h) }))
}
