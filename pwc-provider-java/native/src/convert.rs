//! Conversions between Java values and Rust values.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use bitcoin::consensus::{deserialize, serialize};
use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
use jni::JNIEnv;
use pwc_provider::error::{ErrorCode, PwcError};
use pwc_provider::types::Timestamp;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub const PKG: &str = "org/bitcoinpurity/pwc/provider/";
pub const STRING: &str = "Ljava/lang/String;";
pub const LIST: &str = "Ljava/util/List;";
pub const INSTANT: &str = "Ljava/time/Instant;";
pub const DURATION: &str = "Ljava/time/Duration;";
pub const INTEGER: &str = "Ljava/lang/Integer;";
pub const LONG: &str = "Ljava/lang/Long;";
pub const SHA256: &str = "Lorg/bitcoinj/base/Sha256Hash;";
pub const OUTPOINT: &str = "Lorg/bitcoinj/core/TransactionOutPoint;";
pub const SCRIPT: &str = "Lorg/bitcoinj/script/Script;";
pub const TRANSACTION: &str = "Lorg/bitcoinj/core/Transaction;";

/// Why a native call or a call into Java did not produce a value.
#[derive(Debug)]
pub enum Error {
    Jni(jni::errors::Error),
    /// A `PwcError`, thrown to Java as `PwcException`.
    Pwc(PwcError),
    /// A Java value Rust cannot accept, thrown as `IllegalArgumentException`.
    Invalid(String),
    /// Thrown as `class` with `message`.
    Throw {
        class: &'static str,
        message: String,
    },
    /// Thrown as `class`, built by its `(Kind kind, String message[, String path])` constructor.
    Kind {
        class: &'static str,
        kind: &'static str,
        message: String,
        path: Option<String>,
    },
}

impl From<jni::errors::Error> for Error {
    fn from(e: jni::errors::Error) -> Self {
        Error::Jni(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn invalid(what: impl std::fmt::Display) -> Error {
    Error::Invalid(what.to_string())
}

static LOADER: OnceLock<GlobalRef> = OnceLock::new();
static CLASSES: OnceLock<Mutex<HashMap<String, GlobalRef>>> = OnceLock::new();

/// Remembers the class loader of the SDK, so that threads started by Rust find the SDK and bitcoinj classes.
pub fn init(env: &mut JNIEnv) -> Result<()> {
    let native = env.find_class(format!("{PKG}Native"))?;
    let loader = env
        .call_method(&native, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let _ = LOADER.set(env.new_global_ref(loader)?);
    Ok(())
}

/// The class `name` in JNI form (`java/util/List` or `org/...`), loaded by the SDK's class loader.
pub fn class<'l>(env: &mut JNIEnv<'l>, name: &str) -> Result<JClass<'l>> {
    let classes = CLASSES.get_or_init(|| Mutex::new(HashMap::new()));
    let cached = classes.lock().unwrap().get(name).cloned();
    let global = match cached {
        Some(g) => g,
        None => {
            let loader = LOADER.get().expect("JNI_OnLoad ran");
            let dotted = env.new_string(name.replace('/', "."))?;
            let c = env
                .call_method(
                    loader,
                    "loadClass",
                    "(Ljava/lang/String;)Ljava/lang/Class;",
                    &[(&dotted).into()],
                )?
                .l()?;
            let g = env.new_global_ref(c)?;
            classes.lock().unwrap().insert(name.to_string(), g.clone());
            g
        }
    };
    Ok(JClass::from(env.new_local_ref(global.as_obj())?))
}

pub fn sdk<'l>(env: &mut JNIEnv<'l>, name: &str) -> Result<JClass<'l>> {
    class(env, &format!("{PKG}{name}"))
}

pub fn new<'l>(env: &mut JNIEnv<'l>, name: &str, sig: &str, args: &[JValue]) -> Result<JObject<'l>> {
    let c = sdk(env, name)?;
    Ok(env.new_object(&c, sig, args)?)
}

// ---------------------------------------------------------------------------------------------------------
// Rust to Java

pub fn string<'l>(env: &mut JNIEnv<'l>, s: &str) -> Result<JObject<'l>> {
    Ok(env.new_string(s)?.into())
}

pub fn opt_string<'l>(env: &mut JNIEnv<'l>, s: Option<&str>) -> Result<JObject<'l>> {
    match s {
        Some(s) => string(env, s),
        None => Ok(JObject::null()),
    }
}

pub fn list<'l>(env: &mut JNIEnv<'l>, items: Vec<JObject<'l>>) -> Result<JObject<'l>> {
    let c = class(env, "java/util/ArrayList")?;
    let l = env.new_object(&c, "()V", &[])?;
    for item in items {
        env.call_method(&l, "add", "(Ljava/lang/Object;)Z", &[(&item).into()])?;
    }
    Ok(l)
}

pub fn strings<'l, S: AsRef<str>>(env: &mut JNIEnv<'l>, items: &[S]) -> Result<JObject<'l>> {
    let mut out = Vec::new();
    for s in items {
        out.push(string(env, s.as_ref())?);
    }
    list(env, out)
}

pub fn ints<'l>(env: &mut JNIEnv<'l>, items: impl IntoIterator<Item = i32>) -> Result<JObject<'l>> {
    let mut out = Vec::new();
    for v in items {
        out.push(integer(env, Some(v))?);
    }
    list(env, out)
}

/// The constant of the SDK enum `name` whose name is `value` in upper case, with `-` read as `_`.
pub fn enum_value<'l>(env: &mut JNIEnv<'l>, name: &str, value: &str) -> Result<JObject<'l>> {
    let c = sdk(env, name)?;
    let constant = value.to_uppercase().replace('-', "_");
    Ok(env.get_static_field(&c, constant, format!("L{PKG}{name};"))?.l()?)
}

/// A serde-serialized unit enum as the SDK enum `name`.
pub fn serde_enum<'l, T: serde::Serialize>(env: &mut JNIEnv<'l>, name: &str, value: &T) -> Result<JObject<'l>> {
    let v = serde_json::to_value(value).expect("enum serializes");
    enum_value(env, name, v.as_str().expect("unit enum"))
}

pub fn integer<'l>(env: &mut JNIEnv<'l>, v: Option<i32>) -> Result<JObject<'l>> {
    match v {
        Some(v) => {
            let c = class(env, "java/lang/Integer")?;
            Ok(env
                .call_static_method(&c, "valueOf", "(I)Ljava/lang/Integer;", &[JValue::Int(v)])?
                .l()?)
        }
        None => Ok(JObject::null()),
    }
}

pub fn long<'l>(env: &mut JNIEnv<'l>, v: Option<i64>) -> Result<JObject<'l>> {
    match v {
        Some(v) => {
            let c = class(env, "java/lang/Long")?;
            Ok(env
                .call_static_method(&c, "valueOf", "(J)Ljava/lang/Long;", &[JValue::Long(v)])?
                .l()?)
        }
        None => Ok(JObject::null()),
    }
}

pub fn boolean<'l>(env: &mut JNIEnv<'l>, v: Option<bool>) -> Result<JObject<'l>> {
    match v {
        Some(v) => {
            let c = class(env, "java/lang/Boolean")?;
            Ok(env
                .call_static_method(&c, "valueOf", "(Z)Ljava/lang/Boolean;", &[JValue::Bool(v.into())])?
                .l()?)
        }
        None => Ok(JObject::null()),
    }
}

pub fn u32_int(v: u32) -> Result<i32> {
    i32::try_from(v).map_err(|_| invalid(format!("{v} does not fit a Java int")))
}

pub fn u64_long(v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| invalid(format!("{v} does not fit a Java long")))
}

pub fn instant<'l>(env: &mut JNIEnv<'l>, t: Timestamp) -> Result<JObject<'l>> {
    let millis = (t.0.unix_timestamp_nanos() / 1_000_000) as i64;
    let c = class(env, "java/time/Instant")?;
    Ok(env
        .call_static_method(&c, "ofEpochMilli", "(J)Ljava/time/Instant;", &[JValue::Long(millis)])?
        .l()?)
}

pub fn opt_instant<'l>(env: &mut JNIEnv<'l>, t: Option<Timestamp>) -> Result<JObject<'l>> {
    match t {
        Some(t) => instant(env, t),
        None => Ok(JObject::null()),
    }
}

pub fn duration<'l>(env: &mut JNIEnv<'l>, d: Duration) -> Result<JObject<'l>> {
    let c = class(env, "java/time/Duration")?;
    let nanos = u64_long(d.as_nanos() as u64)?;
    Ok(env
        .call_static_method(&c, "ofNanos", "(J)Ljava/time/Duration;", &[JValue::Long(nanos)])?
        .l()?)
}

pub fn bytes<'l>(env: &mut JNIEnv<'l>, b: &[u8]) -> Result<JObject<'l>> {
    Ok(env.byte_array_from_slice(b)?.into())
}

pub fn sha256<'l>(env: &mut JNIEnv<'l>, hex: &str) -> Result<JObject<'l>> {
    let c = class(env, "org/bitcoinj/base/Sha256Hash")?;
    let s = string(env, hex)?;
    Ok(env
        .call_static_method(&c, "wrap", format!("({STRING}){SHA256}"), &[(&s).into()])?
        .l()?)
}

pub fn opt_sha256<'l>(env: &mut JNIEnv<'l>, hex: Option<&str>) -> Result<JObject<'l>> {
    match hex {
        Some(h) => sha256(env, h),
        None => Ok(JObject::null()),
    }
}

pub fn outpoint<'l>(env: &mut JNIEnv<'l>, o: &bitcoin::OutPoint) -> Result<JObject<'l>> {
    let hash = sha256(env, &o.txid.to_string())?;
    let c = class(env, "org/bitcoinj/core/TransactionOutPoint")?;
    Ok(env.new_object(
        &c,
        format!("(J{SHA256})V"),
        &[JValue::Long(o.vout.into()), (&hash).into()],
    )?)
}

pub fn outpoints<'l>(env: &mut JNIEnv<'l>, items: impl IntoIterator<Item = bitcoin::OutPoint>) -> Result<JObject<'l>> {
    let mut out = Vec::new();
    for o in items {
        out.push(outpoint(env, &o)?);
    }
    list(env, out)
}

pub fn outpoint_set<'l>(env: &mut JNIEnv<'l>, items: &pwc_provider::backend::Reserved) -> Result<JObject<'l>> {
    let c = class(env, "java/util/HashSet")?;
    let set = env.new_object(&c, "()V", &[])?;
    for o in items {
        let j = outpoint(env, o)?;
        env.call_method(&set, "add", "(Ljava/lang/Object;)Z", &[(&j).into()])?;
    }
    Ok(set)
}

pub fn script<'l>(env: &mut JNIEnv<'l>, program: &[u8]) -> Result<JObject<'l>> {
    let c = class(env, "org/bitcoinj/script/Script")?;
    let b = bytes(env, program)?;
    Ok(env
        .call_static_method(&c, "parse", format!("([B){SCRIPT}"), &[(&b).into()])?
        .l()?)
}

pub fn txout<'l>(env: &mut JNIEnv<'l>, value: u64, script: &[u8]) -> Result<JObject<'l>> {
    let coin_class = class(env, "org/bitcoinj/base/Coin")?;
    let coin = env
        .call_static_method(
            &coin_class,
            "valueOf",
            "(J)Lorg/bitcoinj/base/Coin;",
            &[JValue::Long(u64_long(value)?)],
        )?
        .l()?;
    let b = bytes(env, script)?;
    let c = class(env, "org/bitcoinj/core/TransactionOutput")?;
    Ok(env.new_object(
        &c,
        format!("({TRANSACTION}Lorg/bitcoinj/base/Coin;[B)V"),
        &[(&JObject::null()).into(), (&coin).into(), (&b).into()],
    )?)
}

pub fn transaction<'l>(env: &mut JNIEnv<'l>, tx: &bitcoin::Transaction) -> Result<JObject<'l>> {
    let b = bytes(env, &serialize(tx))?;
    let buffers = class(env, "java/nio/ByteBuffer")?;
    let buffer = env
        .call_static_method(&buffers, "wrap", "([B)Ljava/nio/ByteBuffer;", &[(&b).into()])?
        .l()?;
    let c = class(env, "org/bitcoinj/core/Transaction")?;
    Ok(env
        .call_static_method(
            &c,
            "read",
            format!("(Ljava/nio/ByteBuffer;){TRANSACTION}"),
            &[(&buffer).into()],
        )?
        .l()?)
}

/// `new PwcException(code, message, details, retryable)`.
pub fn pwc_exception<'l>(env: &mut JNIEnv<'l>, e: &PwcError) -> Result<JObject<'l>> {
    let code = serde_enum(env, "ErrorCode", &e.code)?;
    let message = string(env, &e.message)?;
    let details = opt_string(env, e.details.as_ref().map(Value::to_string).as_deref())?;
    let retryable = boolean(env, e.retryable)?;
    new(
        env,
        "PwcException",
        &format!("(L{PKG}ErrorCode;{STRING}{STRING}Ljava/lang/Boolean;)V"),
        &[
            (&code).into(),
            (&message).into(),
            (&details).into(),
            (&retryable).into(),
        ],
    )
}

// ---------------------------------------------------------------------------------------------------------
// Java to Rust

pub fn get_string(env: &mut JNIEnv, s: &JObject) -> Result<String> {
    if s.is_null() {
        return Err(invalid("a required value is null"));
    }
    let s: &JString = s.into();
    Ok(env.get_string(s)?.into())
}

pub fn get_opt_string(env: &mut JNIEnv, s: &JObject) -> Result<Option<String>> {
    if s.is_null() {
        return Ok(None);
    }
    get_string(env, s).map(Some)
}

/// Calls the accessor `name` returning an object of type `sig`.
pub fn obj<'l>(env: &mut JNIEnv<'l>, o: &JObject, name: &str, sig: &str) -> Result<JObject<'l>> {
    if o.is_null() {
        return Err(invalid(format!("null instead of an object with {name}()")));
    }
    Ok(env.call_method(o, name, format!("(){sig}"), &[])?.l()?)
}

pub fn str_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<String> {
    let s = obj(env, o, name, STRING)?;
    get_string(env, &s).map_err(|_| invalid(format!("{name} is null")))
}

pub fn opt_str_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Option<String>> {
    let s = obj(env, o, name, STRING)?;
    get_opt_string(env, &s)
}

pub fn long_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<i64> {
    Ok(env.call_method(o, name, "()J", &[])?.j()?)
}

pub fn u64_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<u64> {
    let v = long_field(env, o, name)?;
    u64::try_from(v).map_err(|_| invalid(format!("{name} is negative")))
}

pub fn int_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<i32> {
    Ok(env.call_method(o, name, "()I", &[])?.i()?)
}

pub fn u32_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<u32> {
    let v = int_field(env, o, name)?;
    u32::try_from(v).map_err(|_| invalid(format!("{name} is negative")))
}

pub fn bool_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<bool> {
    Ok(env.call_method(o, name, "()Z", &[])?.z()?)
}

pub fn opt_u32_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Option<u32>> {
    let v = obj(env, o, name, INTEGER)?;
    if v.is_null() {
        return Ok(None);
    }
    let i = env.call_method(&v, "intValue", "()I", &[])?.i()?;
    u32::try_from(i)
        .map(Some)
        .map_err(|_| invalid(format!("{name} is negative")))
}

pub fn opt_u64_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Option<u64>> {
    let v = obj(env, o, name, LONG)?;
    if v.is_null() {
        return Ok(None);
    }
    let l = env.call_method(&v, "longValue", "()J", &[])?.j()?;
    u64::try_from(l)
        .map(Some)
        .map_err(|_| invalid(format!("{name} is negative")))
}

pub fn list_items<'l>(env: &mut JNIEnv<'l>, l: &JObject) -> Result<Vec<JObject<'l>>> {
    if l.is_null() {
        return Err(invalid("a required list is null"));
    }
    let n = env.call_method(l, "size", "()I", &[])?.i()?;
    let mut out = Vec::new();
    for i in 0..n {
        out.push(
            env.call_method(l, "get", "(I)Ljava/lang/Object;", &[JValue::Int(i)])?
                .l()?,
        );
    }
    Ok(out)
}

pub fn list_field<'l>(env: &mut JNIEnv<'l>, o: &JObject, name: &str) -> Result<Vec<JObject<'l>>> {
    let l = obj(env, o, name, LIST)?;
    list_items(env, &l)
}

pub fn string_list(env: &mut JNIEnv, l: &JObject) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for s in list_items(env, l)? {
        out.push(get_string(env, &s)?);
    }
    Ok(out)
}

pub fn string_list_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Vec<String>> {
    let l = obj(env, o, name, LIST)?;
    string_list(env, &l)
}

pub fn int_list(env: &mut JNIEnv, l: &JObject) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for v in list_items(env, l)? {
        if v.is_null() {
            return Err(invalid("a list of integers contains null"));
        }
        let i = env.call_method(&v, "intValue", "()I", &[])?.i()?;
        out.push(u32::try_from(i).map_err(|_| invalid("negative index"))?);
    }
    Ok(out)
}

/// A Java enum constant's name.
pub fn raw_enum_name(env: &mut JNIEnv, e: &JObject) -> Result<String> {
    let n = obj(env, e, "name", STRING)?;
    get_string(env, &n)
}

/// A Java enum constant's name in lower case, with `_` read as `sep`.
pub fn enum_name(env: &mut JNIEnv, e: &JObject, sep: char) -> Result<String> {
    Ok(raw_enum_name(env, e)?.to_lowercase().replace('_', &sep.to_string()))
}

pub fn enum_field(env: &mut JNIEnv, o: &JObject, name: &str, class_name: &str, sep: char) -> Result<String> {
    let e = obj(env, o, name, &format!("L{PKG}{class_name};"))?;
    enum_name(env, &e, sep)
}

/// One of `values`, chosen by a Java enum constant's name.
pub fn static_str(name: &str, values: &[&'static str]) -> Result<&'static str> {
    values
        .iter()
        .find(|v| **v == name)
        .copied()
        .ok_or_else(|| invalid(format!("unexpected value {name}")))
}

/// A serde unit enum from a Java enum constant, whose serde name is the constant's name in lower case with
/// `_` read as `sep`.
pub fn from_serde_enum<T: DeserializeOwned>(env: &mut JNIEnv, e: &JObject, sep: char) -> Result<T> {
    let name = enum_name(env, e, sep)?;
    serde_json::from_value(Value::String(name.clone())).map_err(|_| invalid(format!("unexpected value {name}")))
}

pub fn from_error_code(env: &mut JNIEnv, e: &JObject) -> Result<ErrorCode> {
    let name = raw_enum_name(env, e)?;
    serde_json::from_value(Value::String(name.clone())).map_err(|_| invalid(format!("unexpected error code {name}")))
}

/// A validated string type (`Id`, `Text`, `Network`, `Scope`).
pub fn parse<T: DeserializeOwned>(s: String) -> Result<T> {
    serde_json::from_value(Value::String(s.clone())).map_err(|e| invalid(format!("{s:?}: {e}")))
}

pub fn parse_all<T: DeserializeOwned>(items: Vec<String>) -> Result<Vec<T>> {
    items.into_iter().map(parse).collect()
}

pub fn from_instant(env: &mut JNIEnv, t: &JObject) -> Result<Timestamp> {
    let millis = env.call_method(t, "toEpochMilli", "()J", &[])?.j()?;
    time::OffsetDateTime::from_unix_timestamp_nanos(millis as i128 * 1_000_000)
        .map(Timestamp)
        .map_err(invalid)
}

pub fn opt_instant_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Option<Timestamp>> {
    let t = obj(env, o, name, INSTANT)?;
    if t.is_null() {
        return Ok(None);
    }
    from_instant(env, &t).map(Some)
}

pub fn instant_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Timestamp> {
    opt_instant_field(env, o, name)?.ok_or_else(|| invalid(format!("{name} is null")))
}

pub fn duration_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Duration> {
    let d = obj(env, o, name, DURATION)?;
    if d.is_null() {
        return Err(invalid(format!("{name} is null")));
    }
    let nanos = env.call_method(&d, "toNanos", "()J", &[])?.j()?;
    u64::try_from(nanos)
        .map(Duration::from_nanos)
        .map_err(|_| invalid(format!("{name} is negative")))
}

pub fn get_bytes(env: &mut JNIEnv, b: &JObject) -> Result<Vec<u8>> {
    if b.is_null() {
        return Err(invalid("a required byte array is null"));
    }
    let a: &JByteArray = b.into();
    Ok(env.convert_byte_array(a)?)
}

pub fn bytes_field(env: &mut JNIEnv, o: &JObject, name: &str) -> Result<Vec<u8>> {
    let b = obj(env, o, name, "[B")?;
    get_bytes(env, &b)
}

/// A `Sha256Hash` as lower-case hex in display order.
pub fn from_sha256(env: &mut JNIEnv, h: &JObject) -> Result<String> {
    let s = obj(env, h, "toString", STRING)?;
    get_string(env, &s)
}

pub fn txid(env: &mut JNIEnv, h: &JObject) -> Result<bitcoin::Txid> {
    let s = from_sha256(env, h)?;
    bitcoin::Txid::from_str(&s).map_err(invalid)
}

pub fn from_outpoint(env: &mut JNIEnv, o: &JObject) -> Result<bitcoin::OutPoint> {
    let hash = obj(env, o, "hash", SHA256)?;
    let txid = txid(env, &hash)?;
    let index = long_field(env, o, "index")?;
    let vout = u32::try_from(index).map_err(|_| invalid("outpoint index out of range"))?;
    Ok(bitcoin::OutPoint { txid, vout })
}

pub fn outpoint_list(env: &mut JNIEnv, l: &JObject) -> Result<Vec<bitcoin::OutPoint>> {
    let mut out = Vec::new();
    for o in list_items(env, l)? {
        out.push(from_outpoint(env, &o)?);
    }
    Ok(out)
}

pub fn from_outpoint_set(env: &mut JNIEnv, set: &JObject) -> Result<pwc_provider::backend::Reserved> {
    if set.is_null() {
        return Err(invalid("a required set is null"));
    }
    let array = obj(env, set, "toArray", "[Ljava/lang/Object;")?;
    let array: &jni::objects::JObjectArray = (&array).into();
    let n = env.get_array_length(array)?;
    let mut out = pwc_provider::backend::Reserved::new();
    for i in 0..n {
        let o = env.get_object_array_element(array, i)?;
        out.insert(from_outpoint(env, &o)?);
    }
    Ok(out)
}

pub fn from_script(env: &mut JNIEnv, s: &JObject) -> Result<bitcoin::ScriptBuf> {
    let program = obj(env, s, "program", "[B")?;
    get_bytes(env, &program).map(bitcoin::ScriptBuf::from_bytes)
}

pub fn from_txout(env: &mut JNIEnv, o: &JObject) -> Result<bitcoin::TxOut> {
    let coin = obj(env, o, "getValue", "Lorg/bitcoinj/base/Coin;")?;
    let value = env.call_method(&coin, "getValue", "()J", &[])?.j()?;
    let value = u64::try_from(value).map_err(|_| invalid("negative output value"))?;
    let script = obj(env, o, "getScriptBytes", "[B")?;
    Ok(bitcoin::TxOut {
        value: bitcoin::Amount::from_sat(value),
        script_pubkey: bitcoin::ScriptBuf::from_bytes(get_bytes(env, &script)?),
    })
}

pub fn from_transaction(env: &mut JNIEnv, t: &JObject) -> Result<bitcoin::Transaction> {
    let b = obj(env, t, "serialize", "[B")?;
    let b = get_bytes(env, &b)?;
    deserialize(&b).map_err(invalid)
}

/// The `PwcError` carried by a `PwcException`.
pub fn from_pwc_exception(env: &mut JNIEnv, e: &JObject) -> Result<PwcError> {
    let code = obj(env, e, "code", &format!("L{PKG}ErrorCode;"))?;
    let code = from_error_code(env, &code)?;
    let message = opt_str_field(env, e, "getMessage")?.unwrap_or_default();
    let details = opt_str_field(env, e, "details")?
        .map(|d| serde_json::from_str::<Value>(&d))
        .transpose()
        .map_err(invalid)?;
    let retryable = obj(env, e, "retryable", "Ljava/lang/Boolean;")?;
    let retryable = if retryable.is_null() {
        None
    } else {
        Some(env.call_method(&retryable, "booleanValue", "()Z", &[])?.z()?)
    };
    Ok(PwcError {
        code,
        message,
        details,
        retryable,
    })
}
