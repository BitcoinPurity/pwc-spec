//! `JavaBackend`: the Rust `WalletBackend` trait over a Java `WalletBackend` object.

use jni::objects::{GlobalRef, JObject, JValue};
use jni::JNIEnv;
use pwc_provider::backend::{
    AccountInfo, AddressRecord, Balance, CreateRequest, CreatedPsbt, Reserved, TransactionRecord, Utxo, WalletBackend,
    WalletInfo,
};
use pwc_provider::error::{err, ErrorCode, PwcError};
use pwc_provider::types::{MessageFormat, Text};

use crate::convert::*;
use crate::records::*;

pub struct JavaBackend {
    backend: GlobalRef,
}

/// Why a call into the Java backend gave no value.
enum Failure {
    Pwc(PwcError),
    Other,
}

impl From<Failure> for PwcError {
    fn from(f: Failure) -> PwcError {
        match f {
            Failure::Pwc(e) => e,
            Failure::Other => err(ErrorCode::InternalError, "The wallet backend failed"),
        }
    }
}

const STRING_ARG: &str = "Ljava/lang/String;";
const OPTIONAL: &str = "Ljava/util/Optional;";

/// The value of a Java `Optional`, or `None`.
fn optional<'l>(env: &mut JNIEnv<'l>, o: &JObject) -> Result<Option<JObject<'l>>> {
    if !env.call_method(o, "isPresent", "()Z", &[])?.z()? {
        return Ok(None);
    }
    Ok(Some(env.call_method(o, "get", "()Ljava/lang/Object;", &[])?.l()?))
}

fn items<T>(env: &mut JNIEnv, l: &JObject, f: impl Fn(&mut JNIEnv, &JObject) -> Result<T>) -> Result<Vec<T>> {
    let mut out = Vec::new();
    for item in list_items(env, l)? {
        out.push(f(env, &item)?);
    }
    Ok(out)
}

impl JavaBackend {
    pub fn new(env: &mut JNIEnv, backend: &JObject) -> Result<JavaBackend> {
        Ok(JavaBackend {
            backend: env.new_global_ref(backend)?,
        })
    }

    /// Calls the Java backend on this thread, attached to the JVM as a daemon thread, inside a local frame.
    fn call<T>(
        &self,
        method: &str,
        f: impl FnOnce(&mut JNIEnv, &JObject) -> Result<T>,
    ) -> std::result::Result<T, Failure> {
        let Ok(mut env) = crate::vm().attach_current_thread_as_daemon() else {
            return Err(Failure::Other);
        };
        let outcome = env.with_local_frame(32, |env| -> Result<std::result::Result<T, Failure>> {
            Ok(match f(env, self.backend.as_obj()) {
                Ok(v) => Ok(v),
                Err(e) => Err(failure(env, method, e)),
            })
        });
        outcome.unwrap_or(Err(Failure::Other))
    }

    fn method<'l>(
        &self,
        env: &mut JNIEnv<'l>,
        o: &JObject,
        name: &str,
        sig: &str,
        args: &[JValue],
    ) -> Result<JObject<'l>> {
        Ok(env.call_method(o, name, sig, args)?.l()?)
    }
}

/// A `PwcException` thrown by the backend keeps its content; anything else is printed.
fn failure(env: &mut JNIEnv, method: &str, e: Error) -> Failure {
    if !env.exception_check().unwrap_or(false) {
        eprintln!("pwc-provider: the Java wallet backend's {method} returned an invalid value: {e:?}");
        return Failure::Other;
    }
    let Ok(thrown) = env.exception_occurred() else {
        return Failure::Other;
    };
    let _ = env.exception_clear();
    let is_pwc = sdk(env, "PwcException").and_then(|c| Ok(env.is_instance_of(&thrown, &c)?));
    if let Ok(true) = is_pwc {
        if let Ok(e) = from_pwc_exception(env, &thrown) {
            return Failure::Pwc(e);
        }
    }
    eprintln!("pwc-provider: the Java wallet backend's {method} threw:");
    if env.throw(thrown).is_ok() {
        let _ = env.exception_describe();
    }
    let _ = env.exception_clear();
    Failure::Other
}

fn sdk_type(name: &str) -> String {
    format!("L{PKG}{name};")
}

impl WalletBackend for JavaBackend {
    fn wallets(&self) -> Vec<WalletInfo> {
        self.call("wallets", |env, o| {
            let l = self.method(env, o, "wallets", "()Ljava/util/List;", &[])?;
            items(env, &l, from_wallet_info)
        })
        .unwrap_or_default()
    }

    fn wallet(&self, wallet_id: &str) -> Option<WalletInfo> {
        self.call("wallet", |env, o| {
            let id = string(env, wallet_id)?;
            let w = self.method(env, o, "wallet", &format!("({STRING_ARG}){OPTIONAL}"), &[(&id).into()])?;
            optional(env, &w)?.map(|w| from_wallet_info(env, &w)).transpose()
        })
        .ok()
        .flatten()
    }

    fn account(&self, account_id: &str) -> Option<AccountInfo> {
        self.call("account", |env, o| {
            let id = string(env, account_id)?;
            let a = self.method(env, o, "account", &format!("({STRING_ARG}){OPTIONAL}"), &[(&id).into()])?;
            optional(env, &a)?.map(|a| from_account_info(env, &a)).transpose()
        })
        .ok()
        .flatten()
    }

    fn balance(&self, account_id: &str, reserved: &Reserved) -> std::result::Result<Balance, PwcError> {
        Ok(self.call("balance", |env, o| {
            let id = string(env, account_id)?;
            let set = outpoint_set(env, reserved)?;
            let sig = format!("({STRING_ARG}Ljava/util/Set;){}", sdk_type("Balance"));
            let b = self.method(env, o, "balance", &sig, &[(&id).into(), (&set).into()])?;
            from_balance(env, &b)
        })?)
    }

    fn receive_address(&self, account_id: &str) -> std::result::Result<AddressRecord, PwcError> {
        Ok(self.call("receiveAddress", |env, o| {
            let id = string(env, account_id)?;
            let sig = format!("({STRING_ARG}){}", sdk_type("AddressRecord"));
            let a = self.method(env, o, "receiveAddress", &sig, &[(&id).into()])?;
            from_address_record(env, &a)
        })?)
    }

    fn create_address(&self, account_id: &str, label: Option<&Text>) -> std::result::Result<AddressRecord, PwcError> {
        Ok(self.call("createAddress", |env, o| {
            let id = string(env, account_id)?;
            let label = opt_string(env, label.map(|l| l.as_str()))?;
            let sig = format!("({STRING_ARG}{STRING_ARG}){}", sdk_type("AddressRecord"));
            let a = self.method(env, o, "createAddress", &sig, &[(&id).into(), (&label).into()])?;
            from_address_record(env, &a)
        })?)
    }

    fn addresses(&self, account_id: &str) -> Vec<AddressRecord> {
        self.call("addresses", |env, o| {
            let id = string(env, account_id)?;
            let l = self.method(env, o, "addresses", &format!("({STRING_ARG})Ljava/util/List;"), &[(&id).into()])?;
            items(env, &l, from_address_record)
        })
        .unwrap_or_default()
    }

    fn utxos(&self, account_id: &str, reserved: &Reserved) -> std::result::Result<Vec<Utxo>, PwcError> {
        Ok(self.call("utxos", |env, o| {
            let id = string(env, account_id)?;
            let set = outpoint_set(env, reserved)?;
            let sig = format!("({STRING_ARG}Ljava/util/Set;)Ljava/util/List;");
            let l = self.method(env, o, "utxos", &sig, &[(&id).into(), (&set).into()])?;
            items(env, &l, from_utxo)
        })?)
    }

    fn transactions(&self, account_id: &str) -> std::result::Result<Vec<TransactionRecord>, PwcError> {
        Ok(self.call("transactions", |env, o| {
            let id = string(env, account_id)?;
            let sig = format!("({STRING_ARG})Ljava/util/List;");
            let l = self.method(env, o, "transactions", &sig, &[(&id).into()])?;
            items(env, &l, from_transaction_record)
        })?)
    }

    fn owns_script(&self, account_id: &str, script_pubkey: &bitcoin::Script) -> bool {
        self.call("ownsScript", |env, o| {
            let id = string(env, account_id)?;
            // A script that bitcoinj cannot parse belongs to no account.
            let Ok(s) = script(env, script_pubkey.as_bytes()) else {
                env.exception_clear()?;
                return Ok(false);
            };
            let sig = format!("({STRING_ARG}{SCRIPT})Z");
            Ok(env
                .call_method(o, "ownsScript", sig, &[(&id).into(), (&s).into()])?
                .z()?)
        })
        .unwrap_or(false)
    }

    fn is_unspent(&self, account_id: &str, o: &bitcoin::OutPoint) -> bool {
        self.call("isUnspent", |env, b| {
            let id = string(env, account_id)?;
            let p = outpoint(env, o)?;
            let sig = format!("({STRING_ARG}{OUTPOINT})Z");
            Ok(env.call_method(b, "isUnspent", sig, &[(&id).into(), (&p).into()])?.z()?)
        })
        .unwrap_or(false)
    }

    fn fee_estimate(&self, network: &str, target_blocks: u64) -> Option<u64> {
        self.call("feeEstimate", |env, o| {
            let n = string(env, network)?;
            let blocks = JValue::Long(u64_long(target_blocks)?);
            let sig = format!("({STRING_ARG}J)Ljava/util/OptionalLong;");
            let e = self.method(env, o, "feeEstimate", &sig, &[(&n).into(), blocks])?;
            if !env.call_method(&e, "isPresent", "()Z", &[])?.z()? {
                return Ok(None);
            }
            let v = env.call_method(&e, "getAsLong", "()J", &[])?.j()?;
            u64::try_from(v)
                .map(Some)
                .map_err(|_| invalid("a negative fee estimate"))
        })
        .ok()
        .flatten()
    }

    fn create_psbt(
        &self,
        account_id: &str,
        request: &CreateRequest,
        excluded: &Reserved,
    ) -> std::result::Result<CreatedPsbt, PwcError> {
        Ok(self.call("createPsbt", |env, o| {
            let id = string(env, account_id)?;
            let r = create_request(env, request)?;
            let set = outpoint_set(env, excluded)?;
            let sig = format!(
                "({STRING_ARG}{}Ljava/util/Set;){}",
                sdk_type("CreateRequest"),
                sdk_type("CreatedPsbt")
            );
            let c = self.method(env, o, "createPsbt", &sig, &[(&id).into(), (&r).into(), (&set).into()])?;
            from_created_psbt(env, &c)
        })?)
    }

    fn owned_output(&self, account_id: &str, o: &bitcoin::OutPoint) -> Option<bitcoin::TxOut> {
        self.call("ownedOutput", |env, b| {
            let id = string(env, account_id)?;
            let p = outpoint(env, o)?;
            let sig = format!("({STRING_ARG}{OUTPOINT}){OPTIONAL}");
            let out = self.method(env, b, "ownedOutput", &sig, &[(&id).into(), (&p).into()])?;
            optional(env, &out)?.map(|t| from_txout(env, &t)).transpose()
        })
        .ok()
        .flatten()
    }

    fn sign_psbt(
        &self,
        account_id: &str,
        psbt: &mut bitcoin::Psbt,
        inputs: &[usize],
    ) -> std::result::Result<Vec<u32>, PwcError> {
        let (signed, indexes) = self.call("signPsbt", |env, o| {
            let id = string(env, account_id)?;
            let b = bytes(env, &psbt.serialize())?;
            let mut v = Vec::new();
            for i in inputs {
                v.push(i32::try_from(*i).map_err(|_| invalid("input index out of range"))?);
            }
            let l = ints(env, v)?;
            let sig = format!("({STRING_ARG}[BLjava/util/List;){}", sdk_type("SignedPsbt"));
            let s = self.method(env, o, "signPsbt", &sig, &[(&id).into(), (&b).into(), (&l).into()])?;
            from_signed_psbt(env, &s)
        })?;
        *psbt = signed;
        Ok(indexes)
    }

    fn sign_message(
        &self,
        account_id: &str,
        script_pubkey: &bitcoin::Script,
        message: &str,
        format: MessageFormat,
    ) -> std::result::Result<Vec<u8>, PwcError> {
        Ok(self.call("signMessage", |env, o| {
            let id = string(env, account_id)?;
            let s = script(env, script_pubkey.as_bytes())?;
            let m = string(env, message)?;
            let f = serde_enum(env, "MessageFormat", &format)?;
            let sig = format!("({STRING_ARG}{SCRIPT}{STRING_ARG}{})[B", sdk_type("MessageFormat"));
            let r = self.method(
                env,
                o,
                "signMessage",
                &sig,
                &[(&id).into(), (&s).into(), (&m).into(), (&f).into()],
            )?;
            get_bytes(env, &r)
        })?)
    }

    fn broadcast(&self, network: &str, tx: &bitcoin::Transaction) -> std::result::Result<bitcoin::Txid, PwcError> {
        Ok(self.call("broadcast", |env, o| {
            let n = string(env, network)?;
            let t = transaction(env, tx)?;
            let sig = format!("({STRING_ARG}{TRANSACTION}){SHA256}");
            let h = self.method(env, o, "broadcast", &sig, &[(&n).into(), (&t).into()])?;
            txid(env, &h)
        })?)
    }
}
