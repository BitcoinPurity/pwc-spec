//! The prompt thread, which hands each `Prompt` to the Java dispatcher, and the reply handles.

use std::sync::Mutex;

use jni::objects::{GlobalRef, JObject, JValue};
use jni::JNIEnv;
use pwc_provider::prompt::{GrantChoice, PeerInfo, Prompt, Requester};
use pwc_provider::provider::PromptReceiver;
use pwc_provider::types::{GrantConstraints, PositiveSats};
use tokio::sync::oneshot;

use crate::convert::*;

pub enum Reply {
    Bool(oneshot::Sender<bool>),
    Grant(oneshot::Sender<Option<GrantChoice>>),
}

impl Reply {
    fn is_closed(&self) -> bool {
        match self {
            Reply::Bool(s) => s.is_closed(),
            Reply::Grant(s) => s.is_closed(),
        }
    }
}

/// Taken out when the prompt is answered; dropping it unanswered refuses the prompt.
pub type ReplyHandle = Mutex<Option<Reply>>;

/// Delivers each prompt to `dispatcher.deliver(Prompt)`, in order, until the Provider is gone.
pub fn spawn(mut prompts: PromptReceiver, dispatcher: GlobalRef) {
    std::thread::Builder::new()
        .name("pwc-prompts".into())
        .spawn(move || {
            let mut env = crate::vm()
                .attach_current_thread_as_daemon()
                .expect("attach the prompt thread");
            while let Some(prompt) = prompts.blocking_recv() {
                let _ = env.with_local_frame(64, |env| -> Result<()> {
                    let p = prompt_to_java(env, prompt)?;
                    env.call_method(
                        dispatcher.as_obj(),
                        "deliver",
                        format!("(L{PKG}Prompt;)V"),
                        &[(&p).into()],
                    )?;
                    Ok(())
                });
                if env.exception_check().unwrap_or(false) {
                    let _ = env.exception_describe();
                    let _ = env.exception_clear();
                }
            }
        })
        .expect("start the prompt thread");
}

pub fn peer<'l>(env: &mut JNIEnv<'l>, p: &PeerInfo) -> Result<JObject<'l>> {
    let pid = integer(env, p.pid)?;
    let exe = match &p.executable {
        Some(e) => {
            let s = string(env, &e.to_string_lossy())?;
            let c = class(env, "java/io/File")?;
            let f = env.new_object(&c, format!("({STRING})V"), &[(&s).into()])?;
            env.call_method(&f, "toPath", "()Ljava/nio/file/Path;", &[])?.l()?
        }
        None => JObject::null(),
    };
    new(env, "PeerInfo", &format!("({INTEGER}Ljava/nio/file/Path;)V"), &[(&pid).into(), (&exe).into()])
}

fn requester<'l>(env: &mut JNIEnv<'l>, r: &Requester) -> Result<JObject<'l>> {
    let id = string(env, r.client_id.as_str())?;
    let name = string(env, r.claimed_name.as_str())?;
    let p = peer(env, &r.peer)?;
    new(
        env,
        "Requester",
        &format!("({STRING}{STRING}L{PKG}PeerInfo;)V"),
        &[(&id).into(), (&name).into(), (&p).into()],
    )
}

pub fn constraints<'l>(env: &mut JNIEnv<'l>, c: Option<&GrantConstraints>) -> Result<JObject<'l>> {
    let Some(c) = c else {
        return Ok(JObject::null());
    };
    let max = match c.max_amount_sats_per_request {
        Some(m) => long(env, Some(u64_long(m.0)?))?,
        None => JObject::null(),
    };
    let until = opt_instant(env, c.valid_until)?;
    new(
        env,
        "GrantConstraints",
        &format!("({LONG}{INSTANT})V"),
        &[(&max).into(), (&until).into()],
    )
}

fn prompt_to_java<'l>(env: &mut JNIEnv<'l>, prompt: Prompt) -> Result<JObject<'l>> {
    let text = string(env, &prompt.to_string())?;
    let (class, request_class, request, reply) = match prompt {
        Prompt::Pairing { request: r, reply } => {
            let name = string(env, r.claimed_name.as_str())?;
            let description = opt_string(env, r.claimed_description.as_ref().map(|d| d.as_str()))?;
            let code = string(env, &r.pairing_code)?;
            let p = peer(env, &r.peer)?;
            let request = new(
                env,
                "PairingRequest",
                &format!("({STRING}{STRING}{STRING}L{PKG}PeerInfo;)V"),
                &[(&name).into(), (&description).into(), (&code).into(), (&p).into()],
            )?;
            ("PairingPrompt", "PairingRequest", request, Reply::Bool(reply))
        }
        Prompt::Permissions { request: r, reply } => {
            let req = requester(env, &r.requester)?;
            let scopes = strings(env, &r.scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
            let network = opt_string(env, r.network.as_ref().map(|n| n.as_str()))?;
            let wallets = strings(env, &r.wallet_ids.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
            let accounts = strings(env, &r.account_ids.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
            let expires = opt_instant(env, r.expires_at)?;
            let c = constraints(env, r.constraints.as_ref())?;
            let request = new(
                env,
                "PermissionRequest",
                &format!("(L{PKG}Requester;{LIST}{STRING}{LIST}{LIST}{INSTANT}L{PKG}GrantConstraints;)V"),
                &[
                    (&req).into(),
                    (&scopes).into(),
                    (&network).into(),
                    (&wallets).into(),
                    (&accounts).into(),
                    (&expires).into(),
                    (&c).into(),
                ],
            )?;
            ("PermissionPrompt", "PermissionRequest", request, Reply::Grant(reply))
        }
        Prompt::SignPsbt { request: r, reply } => {
            let req = requester(env, &r.requester)?;
            let account = string(env, r.account_id.as_str())?;
            let network = string(env, &r.network)?;
            let mut inputs = Vec::new();
            for i in &r.inputs_to_sign {
                inputs.push(u32_int(*i)?);
            }
            let inputs = ints(env, inputs)?;
            let mut outputs = Vec::new();
            for o in &r.outputs {
                let address = opt_string(env, o.address.as_deref())?;
                let script = string(env, &o.script_pub_key)?;
                outputs.push(new(
                    env,
                    "OutputSummary",
                    &format!("({STRING}{STRING}JZ)V"),
                    &[
                        (&address).into(),
                        (&script).into(),
                        JValue::Long(u64_long(o.amount_sats)?),
                        JValue::Bool(o.to_this_account.into()),
                    ],
                )?);
            }
            let outputs = list(env, outputs)?;
            let psbt = bytes(env, &r.psbt)?;
            let input_count = i32::try_from(r.input_count).map_err(|_| invalid("too many inputs"))?;
            let request = new(
                env,
                "PsbtSignRequest",
                &format!("(L{PKG}Requester;{STRING}{STRING}{LIST}I{LIST}JJ[B)V"),
                &[
                    (&req).into(),
                    (&account).into(),
                    (&network).into(),
                    (&inputs).into(),
                    JValue::Int(input_count),
                    (&outputs).into(),
                    JValue::Long(u64_long(r.fee_sats)?),
                    JValue::Long(u64_long(r.fee_rate_sat_per_kvb)?),
                    (&psbt).into(),
                ],
            )?;
            ("PsbtSignPrompt", "PsbtSignRequest", request, Reply::Bool(reply))
        }
        Prompt::SignMessage { request: r, reply } => {
            let req = requester(env, &r.requester)?;
            let account = string(env, r.account_id.as_str())?;
            let network = string(env, &r.network)?;
            let address = string(env, &r.address)?;
            let message = string(env, &r.message)?;
            let format = serde_enum(env, "MessageFormat", &r.format)?;
            let request = new(
                env,
                "MessageSignRequest",
                &format!("(L{PKG}Requester;{STRING}{STRING}{STRING}{STRING}L{PKG}MessageFormat;)V"),
                &[
                    (&req).into(),
                    (&account).into(),
                    (&network).into(),
                    (&address).into(),
                    (&message).into(),
                    (&format).into(),
                ],
            )?;
            ("MessageSignPrompt", "MessageSignRequest", request, Reply::Bool(reply))
        }
    };
    let handle = Box::into_raw(Box::new(ReplyHandle::new(Some(reply))));
    let made = new(
        env,
        class,
        &format!("(J{STRING}L{PKG}{request_class};)V"),
        &[JValue::Long(handle as i64), (&text).into(), (&request).into()],
    );
    if made.is_err() {
        // SAFETY: the Java object that would own the handle was not created.
        drop(unsafe { Box::from_raw(handle) });
    }
    made
}

/// The handle behind a Java prompt's pointer.
///
/// # Safety
/// `ptr` comes from `prompt_to_java` and has not been freed.
pub unsafe fn handle<'a>(ptr: i64) -> &'a ReplyHandle {
    &*(ptr as *const ReplyHandle)
}

pub fn is_withdrawn(h: &ReplyHandle) -> bool {
    h.lock().unwrap().as_ref().is_some_and(Reply::is_closed)
}

/// Sends the answer; false if the prompt was already answered.
pub fn answer(h: &ReplyHandle, approve: bool) -> bool {
    match h.lock().unwrap().take() {
        Some(Reply::Bool(s)) => {
            let _ = s.send(approve);
            true
        }
        Some(Reply::Grant(s)) => {
            let _ = s.send(None);
            true
        }
        None => false,
    }
}

pub fn grant_choice(env: &mut JNIEnv, c: &JObject) -> Result<GrantChoice> {
    let constraints = obj(env, c, "constraints", &format!("L{PKG}GrantConstraints;"))?;
    let constraints = if constraints.is_null() {
        None
    } else {
        let max = opt_u64_field(env, &constraints, "maxAmountSatsPerRequest")?
            .map(|m| parse::<PositiveSats>(m.to_string()))
            .transpose()?;
        Some(GrantConstraints {
            max_amount_sats_per_request: max,
            valid_until: opt_instant_field(env, &constraints, "validUntil")?,
        })
    };
    Ok(GrantChoice {
        network: parse(str_field(env, c, "network")?)?,
        scopes: parse_all(string_list_field(env, c, "scopes")?)?,
        wallet_ids: parse_all(string_list_field(env, c, "walletIds")?)?,
        account_ids: parse_all(string_list_field(env, c, "accountIds")?)?,
        expires_at: opt_instant_field(env, c, "expiresAt")?,
        constraints,
    })
}

/// Sends a grant choice; false if the prompt was already answered. Only a permission prompt takes one.
pub fn grant(h: &ReplyHandle, choice: GrantChoice) -> Result<bool> {
    let mut reply = h.lock().unwrap();
    match reply.take() {
        Some(Reply::Grant(s)) => {
            let _ = s.send(Some(choice));
            Ok(true)
        }
        Some(other) => {
            *reply = Some(other);
            Err(invalid("only a permission prompt takes a grant choice"))
        }
        None => Ok(false),
    }
}
