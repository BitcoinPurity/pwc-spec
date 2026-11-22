//! JNI binding of `pwc-provider` for the Java Provider SDK. Every exported function is a static method of
//! `org.bitcoinpurity.pwc.provider.Native`.

mod backend;
mod convert;
mod memory;
mod prompts;
mod provider;
mod records;

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use jni::objects::{JObject, JValue};
use jni::sys::{jint, JNI_VERSION_1_8};
use jni::{JNIEnv, JavaVM};
use pwc_provider::provider::{Config, Limits, RateLimit};
use pwc_provider::types::FeeRate;
use tokio::runtime::Runtime;

use convert::*;

static VM: OnceLock<JavaVM> = OnceLock::new();
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut c_void) -> jint {
    let mut env = vm.get_env().expect("JNI_OnLoad runs on a Java thread");
    if init(&mut env).is_err() {
        return jni::sys::JNI_ERR;
    }
    let _ = VM.set(vm);
    JNI_VERSION_1_8
}

pub(crate) fn vm() -> &'static JavaVM {
    VM.get().expect("JNI_OnLoad ran")
}

/// The runtime that runs every server, connection and timer.
pub(crate) fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("pwc-runtime")
            .build()
            .expect("tokio runtime")
    })
}

/// Runs a native method body: inside the runtime, with Rust errors and panics thrown as Java exceptions.
pub(crate) fn native<'l, T>(env: &mut JNIEnv<'l>, default: T, f: impl FnOnce(&mut JNIEnv<'l>) -> Result<T>) -> T {
    let _guard = runtime().enter();
    let outcome = catch_unwind(AssertUnwindSafe(|| f(env)));
    let error = match outcome {
        Ok(Ok(v)) => return v,
        Ok(Err(e)) => e,
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic in the native library".into());
            Error::Throw {
                class: "java/lang/IllegalStateException",
                message,
            }
        }
    };
    throw(env, error);
    default
}

fn throw(env: &mut JNIEnv, error: Error) {
    if env.exception_check().unwrap_or(true) {
        return;
    }
    let built = match error {
        Error::Jni(e) => Err(("java/lang/IllegalStateException", e.to_string())),
        Error::Invalid(m) => Err(("java/lang/IllegalArgumentException", m)),
        Error::Throw { class, message } => Err((class, message)),
        Error::Pwc(e) => Ok(pwc_exception(env, &e)),
        Error::Kind {
            class,
            kind,
            message,
            path,
        } => Ok(kinded_exception(env, class, kind, &message, path.as_deref())),
    };
    // If building the exception object failed, that failure is already pending in Java.
    let _ = match built {
        Ok(Ok(o)) => env.throw(jni::objects::JThrowable::from(o)),
        Ok(Err(_)) => Ok(()),
        Err((class, message)) => env.throw_new(class, message),
    };
}

/// `new <class>(<class>.Kind kind, String message[, String path])`.
fn kinded_exception<'l>(
    env: &mut JNIEnv<'l>,
    class: &str,
    kind: &str,
    message: &str,
    path: Option<&str>,
) -> Result<JObject<'l>> {
    let kind_class = format!("{class}$Kind");
    let k = class_constant(env, &kind_class, kind)?;
    let m = string(env, message)?;
    let c = convert::class(env, class)?;
    match path {
        Some(p) => {
            let p = string(env, p)?;
            Ok(env.new_object(
                &c,
                format!("(L{kind_class};{STRING}{STRING})V"),
                &[(&k).into(), (&m).into(), (&p).into()],
            )?)
        }
        None => Ok(env.new_object(&c, format!("(L{kind_class};{STRING})V"), &[(&k).into(), (&m).into()])?),
    }
}

fn class_constant<'l>(env: &mut JNIEnv<'l>, class: &str, constant: &str) -> Result<JObject<'l>> {
    let c = convert::class(env, class)?;
    Ok(env.get_static_field(&c, constant, format!("L{class};"))?.l()?)
}

// ---------------------------------------------------------------------------------------------------------
// Config

fn rate_limit<'l>(env: &mut JNIEnv<'l>, r: RateLimit) -> Result<JObject<'l>> {
    let window = duration(env, r.window)?;
    let max = i32::try_from(r.max).map_err(|_| invalid("rate limit too large"))?;
    new(
        env,
        "RateLimit",
        &format!("(I{DURATION})V"),
        &[JValue::Int(max), (&window).into()],
    )
}

fn config_to_java<'l>(env: &mut JNIEnv<'l>, c: &Config) -> Result<JObject<'l>> {
    let l = &c.limits;
    let values = [
        l.max_page_size,
        l.max_psbt_bytes,
        l.max_combine_psbts,
        l.max_raw_transaction_bytes,
        l.max_message_bytes,
        l.max_concurrent_operations,
        l.operation_retention_seconds,
        l.high_fee_rate.0,
    ];
    let mut args = Vec::new();
    for v in values {
        args.push(JValue::Long(u64_long(v)?));
    }
    let limits = new(env, "Limits", "(JJJJJJJJ)V", &args)?;
    let name = string(env, &c.provider_name)?;
    let version = string(env, &c.provider_version)?;
    let hello = duration(env, c.hello_timeout)?;
    let auth = duration(env, c.auth_timeout)?;
    let pairing = duration(env, c.pairing_timeout)?;
    let approval = duration(env, c.approval_timeout)?;
    let reservation = duration(env, c.reservation_ttl)?;
    let pairing_limit = rate_limit(env, c.pairing_limit)?;
    let auth_limit = rate_limit(env, c.auth_failure_limit)?;
    let auth_delay = duration(env, c.auth_failure_delay)?;
    let prompt_limit = rate_limit(env, c.prompt_limit)?;
    let buffer = i32::try_from(c.event_buffer).map_err(|_| invalid("event buffer too large"))?;
    let rl = format!("L{PKG}RateLimit;");
    new(
        env,
        "Config",
        &format!(
            "({STRING}{STRING}L{PKG}Limits;{DURATION}{DURATION}{DURATION}{DURATION}{DURATION}{rl}{rl}{DURATION}{rl}I)V"
        ),
        &[
            (&name).into(),
            (&version).into(),
            (&limits).into(),
            (&hello).into(),
            (&auth).into(),
            (&pairing).into(),
            (&approval).into(),
            (&reservation).into(),
            (&pairing_limit).into(),
            (&auth_limit).into(),
            (&auth_delay).into(),
            (&prompt_limit).into(),
            JValue::Int(buffer),
        ],
    )
}

fn rate_limit_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<RateLimit> {
    let r = obj(env, o, name, &format!("L{PKG}RateLimit;"))?;
    let max = int_field(env, &r, "max")?;
    Ok(RateLimit {
        max: usize::try_from(max).map_err(|_| invalid(format!("{name}.max is negative")))?,
        window: duration_field(env, &r, "window")?,
    })
}

pub(crate) fn config_from_java(env: &mut JNIEnv, c: &JObject) -> Result<Config> {
    let l = obj(env, c, "limits", &format!("L{PKG}Limits;"))?;
    let limits = Limits {
        max_page_size: u64_field(env, &l, "maxPageSize")?,
        max_psbt_bytes: u64_field(env, &l, "maxPsbtBytes")?,
        max_combine_psbts: u64_field(env, &l, "maxCombinePsbts")?,
        max_raw_transaction_bytes: u64_field(env, &l, "maxRawTransactionBytes")?,
        max_message_bytes: u64_field(env, &l, "maxMessageBytes")?,
        max_concurrent_operations: u64_field(env, &l, "maxConcurrentOperations")?,
        operation_retention_seconds: u64_field(env, &l, "operationRetentionSeconds")?,
        high_fee_rate: FeeRate(u64_field(env, &l, "highFeeRate")?),
    };
    let event_buffer = int_field(env, c, "eventBuffer")?;
    Ok(Config {
        provider_name: str_field(env, c, "providerName")?,
        provider_version: str_field(env, c, "providerVersion")?,
        limits,
        hello_timeout: duration_field(env, c, "helloTimeout")?,
        auth_timeout: duration_field(env, c, "authTimeout")?,
        pairing_timeout: duration_field(env, c, "pairingTimeout")?,
        approval_timeout: duration_field(env, c, "approvalTimeout")?,
        reservation_ttl: duration_field(env, c, "reservationTtl")?,
        pairing_limit: rate_limit_field(env, c, "pairingLimit")?,
        auth_failure_limit: rate_limit_field(env, c, "authFailureLimit")?,
        auth_failure_delay: duration_field(env, c, "authFailureDelay")?,
        prompt_limit: rate_limit_field(env, c, "promptLimit")?,
        event_buffer: usize::try_from(event_buffer)
            .ok()
            .filter(|b| *b > 0)
            .ok_or_else(|| invalid("eventBuffer must be positive"))?,
    })
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_defaultConfig<'l>(
    mut env: JNIEnv<'l>,
    _: JObject<'l>,
) -> jni::sys::jobject {
    native(&mut env, std::ptr::null_mut(), |env| {
        Ok(config_to_java(env, &Config::default())?.into_raw())
    })
}
