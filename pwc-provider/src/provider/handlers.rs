//! Check 10 of Core §8: method-specific checks and execution.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::grants::{self, Grant};
use super::operations::Reservation;
use super::{Inner, OpState, Outcome, Provider, Target};
use crate::backend::CreateRequest;
use crate::backend::{AccountInfo, WalletInfo};
use crate::error::{err, ErrorCode, PwcError};
use crate::message;
use crate::methods::{Listing, Request, RequestPermissions};
use crate::prompt::{GrantChoice, PermissionRequest, Prompt};
use crate::prompt::{MessageSignRequest, OutputSummary, PsbtSignRequest};
use crate::types::{chain_network, Base64, FeePolicy, Hex, Id, MessageFormat, Sats, Timestamp};

pub(super) struct Ctx<'a> {
    pub client_id: &'a Id,
    pub session_id: &'a Id,
    pub now: Timestamp,
    pub target: Option<&'a Target>,
}

impl Ctx<'_> {
    fn account(&self) -> &AccountInfo {
        self.target
            .and_then(|t| t.account.as_ref())
            .expect("account resolved at check 7")
    }

    fn wallet(&self) -> &WalletInfo {
        &self.target.expect("wallet resolved at check 7").wallet
    }
}

fn parse_address(s: &str, network: bitcoin::Network) -> Result<bitcoin::Address, PwcError> {
    let address: bitcoin::Address<bitcoin::address::NetworkUnchecked> = s
        .parse()
        .map_err(|_| err(ErrorCode::InvalidArgument, "Not a valid address"))?;
    address
        .require_network(network)
        .map_err(|_| err(ErrorCode::NetworkMismatch, "The address is encoded for another network"))
}

/// Core §8.4: a format absent from `message_formats`, or one that cannot sign this address type.
fn format_signs(format: MessageFormat, address: &bitcoin::Address) -> Result<(), PwcError> {
    let spk = address.script_pubkey();
    match format {
        MessageFormat::Bip322Simple if spk.is_p2wpkh() => Ok(()),
        MessageFormat::Legacy if spk.is_p2pkh() => Ok(()),
        MessageFormat::Bip322Full => Err(err(ErrorCode::UnsupportedOperation, "bip322-full is not supported")),
        _ => Err(err(
            ErrorCode::UnsupportedOperation,
            "The format cannot sign this address type",
        )),
    }
}

fn account_summary(a: &AccountInfo) -> Value {
    json!({
        "account_id": a.account_id,
        "wallet_id": a.wallet_id,
        "display_name": a.display_name,
        "network": a.network,
        "can_sign": a.can_sign,
        "script_types": [a.script_type],
        "methods": a.methods,
    })
}

impl Provider {
    pub(super) fn execute(
        self: &Arc<Self>,
        inner: &mut Inner,
        ctx: &Ctx,
        request: Request,
    ) -> Result<Outcome, PwcError> {
        match request {
            Request::GetCapabilities => Ok(Outcome::Value(self.capabilities())),
            Request::GetInfo => {
                let grant_ids: Vec<&Id> = self
                    .active_grants(inner, ctx.client_id, ctx.now)
                    .map(|g| &g.grant_id)
                    .collect();
                let session = &inner.sessions[ctx.session_id];
                Ok(Outcome::Value(json!({
                    "session_id": ctx.session_id,
                    "client_id": ctx.client_id,
                    "grant_ids": grant_ids,
                    "created_at": session.created_at,
                })))
            }
            Request::RequestPermissions(p) => Ok(self.request_permissions(inner, ctx, p)),
            Request::GetPermissions => {
                let grants: Vec<Value> = inner
                    .state
                    .grants
                    .values()
                    .filter(|g| &g.client_id == ctx.client_id)
                    .map(|g| g.to_json(ctx.now))
                    .collect();
                Ok(Outcome::Value(json!({ "grants": grants })))
            }
            Request::RevokePermissions(p) => {
                let owned = inner
                    .state
                    .grants
                    .get(p.grant_id.as_str())
                    .is_some_and(|g| &g.client_id == ctx.client_id);
                if !owned {
                    return Err(err(ErrorCode::GrantNotFound, "Grant not found"));
                }
                let revoked = self.revoke(inner, p.grant_id.as_str());
                Ok(Outcome::Value(json!({ "revoked": revoked })))
            }
            Request::WalletList => {
                let grants: Vec<&Grant> = self
                    .active_grants(inner, ctx.client_id, ctx.now)
                    .filter(|g| g.has_scope("wallets:read"))
                    .collect();
                let wallets: Vec<Value> = self
                    .backend
                    .wallets()
                    .into_iter()
                    .filter(|w| grants.iter().any(|g| g.covers_wallet(w)))
                    .map(|w| {
                        let readable = super::wallet_readable(&w).is_ok();
                        let account_ids: Vec<&Id> = w
                            .account_ids
                            .iter()
                            .filter(|a| {
                                readable
                                    && self
                                        .backend
                                        .account(a.as_str())
                                        .is_some_and(|a| grants.iter().any(|g| g.covers_account(&a)))
                            })
                            .collect();
                        json!({
                            "wallet_id": w.wallet_id,
                            "display_name": w.display_name,
                            "network": w.network,
                            "wallet_kind": w.kind,
                            "state": w.state,
                            "account_ids": account_ids,
                        })
                    })
                    .collect();
                Ok(Outcome::Value(json!({ "wallets": wallets })))
            }
            Request::AccountList(_) => {
                let accounts: Vec<Value> = ctx
                    .wallet()
                    .account_ids
                    .iter()
                    .filter_map(|a| self.backend.account(a.as_str()))
                    .filter(|a| {
                        self.active_grants(inner, ctx.client_id, ctx.now)
                            .any(|g| g.has_scope("wallets:read") && g.covers_account(a))
                    })
                    .map(|a| account_summary(&a))
                    .collect();
                Ok(Outcome::AccountData(json!({ "accounts": accounts })))
            }
            Request::OperationGet(p) => {
                self.own_operation(inner, ctx.client_id, &p.operation_id)?;
                Ok(Outcome::Operation(p.operation_id))
            }
            Request::OperationCancel(p) => {
                let state = self.own_operation(inner, ctx.client_id, &p.operation_id)?;
                if state == OpState::Processing {
                    return Err(err(
                        ErrorCode::OperationNotCancelable,
                        "The Operation is past an irreversible step",
                    ));
                }
                let e = err(ErrorCode::OperationCanceled, "The Operation was canceled");
                self.transition(inner, &p.operation_id, OpState::Canceled, None, Some(e));
                Ok(Outcome::Operation(p.operation_id))
            }
            Request::GetBalance(_) => {
                let reserved = self.reserved(inner, &ctx.account().account_id, ctx.now);
                let b = self.backend.balance(ctx.account().account_id.as_str(), &reserved)?;
                Ok(Outcome::AccountData(json!(b)))
            }
            Request::GetReceiveAddress(_) => Ok(Outcome::AccountData(json!(self
                .backend
                .receive_address(ctx.account().account_id.as_str())?))),
            Request::AddressCreate(p) => {
                self.account_method(ctx, "address.create")?;
                let a = self
                    .backend
                    .create_address(ctx.account().account_id.as_str(), p.label.as_ref())?;
                Ok(Outcome::AccountData(json!(a)))
            }
            Request::AddressList(p) => {
                let items: Vec<Value> = self
                    .backend
                    .addresses(ctx.account().account_id.as_str())
                    .iter()
                    .map(|a| json!(a))
                    .collect();
                self.page(inner, ctx, "address.list", &p, items, "addresses")
            }
            Request::UtxoList(p) => {
                let reserved = self.reserved(inner, &ctx.account().account_id, ctx.now);
                let items: Vec<Value> = self
                    .backend
                    .utxos(ctx.account().account_id.as_str(), &reserved)?
                    .iter()
                    .map(|u| json!(u))
                    .collect();
                self.page(inner, ctx, "utxo.list", &p, items, "utxos")
            }
            Request::TransactionList(p) => {
                let items: Vec<Value> = self
                    .backend
                    .transactions(ctx.account().account_id.as_str())?
                    .iter()
                    .map(|t| json!(t))
                    .collect();
                self.page(inner, ctx, "transaction.list", &p, items, "transactions")
            }
            Request::TransactionGet(p) => {
                let found = self
                    .backend
                    .transactions(ctx.account().account_id.as_str())?
                    .into_iter()
                    .find(|t| t.txid == p.txid);
                match found {
                    Some(t) => Ok(Outcome::AccountData(json!({ "transaction": t }))),
                    None => Err(err(
                        ErrorCode::TransactionNotFound,
                        "The account has no record of this transaction",
                    )),
                }
            }
            Request::PsbtCombine(p) => {
                let parts: Vec<Vec<u8>> = p.psbts.into_iter().map(|b| b.0).collect();
                let merged = crate::psbt::combine(&parts)?;
                Ok(Outcome::Value(json!({ "psbt": Base64(merged), "psbt_version": 0 })))
            }
            Request::PsbtFinalize(p) => {
                let f = crate::psbt::finalize(&p.psbt.0)?;
                let mut v = json!({ "psbt": Base64(f.psbt), "psbt_version": 0, "complete": f.transaction.is_some() });
                if let Some(tx) = f.transaction {
                    v["raw_transaction"] = json!(Hex(bitcoin::consensus::serialize(&tx)));
                    v["txid"] = json!(tx.compute_txid().to_string());
                }
                Ok(Outcome::Value(v))
            }
            Request::PsbtCreate(p) => self.psbt_create(inner, ctx, p),
            Request::ReleaseReservation(p) => {
                let not_found = || err(ErrorCode::ReservationNotFound, "Reservation not found");
                let r = inner.state.reservations.get(&p.reservation_id).ok_or_else(not_found)?;
                if &r.client_id != ctx.client_id || r.account_id != ctx.account().account_id {
                    return Err(not_found());
                }
                let released = self.reservation_active(inner, r, ctx.now);
                inner
                    .state
                    .reservations
                    .get_mut(&p.reservation_id)
                    .expect("found above")
                    .released = true;
                Ok(Outcome::Value(json!({ "released": released })))
            }
            Request::Broadcast(p) => {
                self.account_method(ctx, "transaction.broadcast")?;
                let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&p.raw_transaction.0)
                    .map_err(|_| err(ErrorCode::InvalidArgument, "raw_transaction is not a valid transaction"))?;
                let outcome = match self.backend.broadcast(&ctx.account().network, &tx) {
                    Ok(txid) => {
                        let account = ctx.account();
                        let a = super::Audience::Account {
                            account,
                            scope: "balances:read",
                        };
                        self.emit(
                            inner,
                            "account.balanceChanged",
                            Some(&account.account_id),
                            None,
                            json!({}),
                            a,
                        );
                        let a = super::Audience::Account {
                            account,
                            scope: "transactions:read",
                        };
                        let payload = json!({ "txid": txid.to_string(), "status": "unconfirmed" });
                        self.emit(
                            inner,
                            "account.transactionChanged",
                            Some(&account.account_id),
                            None,
                            payload,
                            a,
                        );
                        Ok(json!({ "txid": txid.to_string() }))
                    }
                    Err(e) if matches!(e.code, ErrorCode::TransactionRejected | ErrorCode::NetworkUnavailable) => {
                        Err(e)
                    }
                    Err(e) => return Err(e),
                };
                Ok(Outcome::Operation(self.finished_operation(
                    inner,
                    ctx,
                    "transaction.broadcast",
                    outcome,
                )))
            }
            Request::PsbtSign(p) => self.psbt_sign(inner, ctx, p),
            Request::MessageSign(p) => self.message_sign(inner, ctx, p),
            Request::MessageVerify(p) => {
                let net = chain_network(p.network.as_str())
                    .ok_or_else(|| err(ErrorCode::UnsupportedOperation, "The network is not supported"))?;
                let address = parse_address(p.address.as_str(), net)?;
                format_signs(p.format, &address)?;
                let secp = bitcoin::secp256k1::Secp256k1::verification_only();
                let valid = match p.format {
                    MessageFormat::Bip322Simple => {
                        message::verify_bip322_simple(&secp, &address, p.message.as_bytes(), &p.signature.0)
                    }
                    _ => message::verify_legacy(&secp, &address, &p.message, &p.signature.0),
                }
                .ok_or_else(|| err(ErrorCode::InvalidArgument, "Malformed signature"))?;
                Ok(Outcome::Value(json!({ "valid": valid })))
            }
        }
    }

    /// The grant under which an amount is allowed: an active grant of the Client holding `scope` for the
    /// account whose `max_amount_sats_per_request` admits `outflow` (Core §4.8).
    fn grant_for_amount(&self, inner: &Inner, ctx: &Ctx, scope: &str, outflow: u64) -> Result<Id, PwcError> {
        self.active_grants(inner, ctx.client_id, ctx.now)
            .filter(|g| g.has_scope(scope) && g.covers_account(ctx.account()))
            .find(|g| {
                let limit = g.constraints.as_ref().and_then(|c| c.max_amount_sats_per_request);
                limit.is_none_or(|l| outflow <= l.0)
            })
            .map(|g| g.grant_id.clone())
            .ok_or_else(|| {
                err(
                    ErrorCode::GrantConstraintViolated,
                    "The amount exceeds the grant's max_amount_sats_per_request",
                )
            })
    }

    fn psbt_create(&self, inner: &mut Inner, ctx: &Ctx, p: crate::methods::PsbtCreate) -> Result<Outcome, PwcError> {
        self.account_method(ctx, "psbt.create")?;
        let account = ctx.account();
        let network = chain_network(&account.network).expect("supported network");
        let mut outputs = Vec::new();
        for o in &p.outputs {
            outputs.push((
                parse_address(o.address.as_str(), network)?.script_pubkey(),
                o.amount_sats.0,
            ));
        }
        let fee_rate = match &p.fee_policy {
            FeePolicy::Rate { sat_per_kvb } => sat_per_kvb.0,
            FeePolicy::WalletEstimate { target_blocks } => self
                .backend
                .fee_estimate(&account.network, target_blocks.0)
                .ok_or_else(|| err(ErrorCode::StateUnavailable, "The wallet cannot estimate a fee now"))?,
        };
        let selected = p.selected_utxos.as_ref().map(|list| {
            list.iter()
                .map(|o| bitcoin::OutPoint {
                    txid: o.txid.as_str().parse().expect("Hash256 is a txid"),
                    vout: o.vout,
                })
                .collect()
        });
        let request = CreateRequest {
            outputs,
            fee_rate,
            selected,
            replaceable: p.replaceable.unwrap_or(true),
        };
        let excluded = self.reserved(inner, &account.account_id, ctx.now);
        let created = self
            .backend
            .create_psbt(account.account_id.as_str(), &request, &excluded)?;
        let inputs: u64 = created
            .psbt
            .inputs
            .iter()
            .filter_map(|i| i.witness_utxo.as_ref())
            .map(|o| o.value.to_sat())
            .sum();
        let back: u64 = created
            .psbt
            .unsigned_tx
            .output
            .iter()
            .filter(|o| self.backend.owns_script(account.account_id.as_str(), &o.script_pubkey))
            .map(|o| o.value.to_sat())
            .sum();
        let grant_id = self.grant_for_amount(inner, ctx, "psbt:create", inputs.saturating_sub(back))?;
        let reservation_id = Id::random("rs");
        let until = ctx
            .now
            .after(self.config.reservation_ttl.min(std::time::Duration::from_secs(86_400)));
        inner.state.reservations.insert(
            reservation_id.clone(),
            Reservation {
                client_id: ctx.client_id.clone(),
                account_id: account.account_id.clone(),
                grant_id,
                outpoints: created.inputs.clone(),
                until,
                released: false,
            },
        );
        let result = json!({
            "account_id": account.account_id,
            "network": account.network,
            "psbt": Base64(created.psbt.serialize()),
            "psbt_version": 0,
            "fee_sats": Sats(created.fee),
            "vsize": created.vsize,
            "change_output_indexes": created.change_output_indexes,
            "reservation_id": reservation_id,
            "reserved_until": until,
        });
        Ok(Outcome::Operation(self.finished_operation(
            inner,
            ctx,
            "psbt.create",
            Ok(result),
        )))
    }

    /// Core §8.3 `psbt.sign`, checked in the order parsing and version, `input_indexes`, signability, script
    /// types and sighash modes, review validation, grant constraints, high-fee warning.
    fn psbt_sign(
        self: &Arc<Self>,
        inner: &mut Inner,
        ctx: &Ctx,
        p: crate::methods::PsbtSign,
    ) -> Result<Outcome, PwcError> {
        self.account_method(ctx, "psbt.sign")?;
        let account = ctx.account().clone();
        let id = account.account_id.as_str();
        let mut psbt = crate::psbt::parse(&p.psbt.0)?;
        let n = psbt.inputs.len();
        if p.input_indexes
            .as_ref()
            .is_some_and(|idx| idx.iter().any(|&i| i as usize >= n))
        {
            return Err(err(ErrorCode::InvalidArgument, "input_indexes out of range"));
        }
        let owned: Vec<Option<bitcoin::TxOut>> = psbt
            .unsigned_tx
            .input
            .iter()
            .map(|txin| self.backend.owned_output(id, &txin.previous_output))
            .collect();
        let cannot = || {
            err(
                ErrorCode::UnsupportedOperation,
                "The account cannot sign the requested inputs",
            )
        };
        let targets: Vec<usize> = match &p.input_indexes {
            Some(idx) => {
                let mut t: Vec<usize> = idx.iter().map(|&i| i as usize).collect();
                t.sort_unstable();
                if t.iter().any(|&i| owned[i].is_none()) {
                    return Err(cannot());
                }
                t
            }
            None => (0..n).filter(|&i| owned[i].is_some()).collect(),
        };
        if targets.is_empty() {
            return Err(cannot());
        }
        let mut nondefault_sighash = false;
        for &i in &targets {
            if !owned[i].as_ref().unwrap().script_pubkey.is_p2wpkh() {
                return Err(err(ErrorCode::UnsupportedScript, "Unsupported script type"));
            }
            if let Some(t) = psbt.inputs[i].sighash_type {
                match t.ecdsa_hash_ty() {
                    Ok(bitcoin::EcdsaSighashType::All) => {}
                    Ok(_) => nondefault_sighash = true,
                    Err(_) => return Err(err(ErrorCode::UnsupportedScript, "Unsupported sighash mode")),
                }
            }
        }
        let mut prevouts = Vec::with_capacity(n);
        for (i, real) in owned.iter().enumerate() {
            let claimed = crate::psbt::previous_output(&psbt, i)?;
            match real {
                Some(r) if claimed.as_ref().is_some_and(|c| c != r) => {
                    return Err(err(
                        ErrorCode::PsbtValidationFailed,
                        "Previous-output data does not match the wallet",
                    ));
                }
                Some(r) => prevouts.push(Some(r.clone())),
                None => prevouts.push(claimed),
            }
        }
        let out_total: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
        let in_total: Option<u64> = prevouts.iter().map(|p| p.as_ref().map(|o| o.value.to_sat())).sum();
        if in_total.is_some_and(|t| out_total > t) {
            return Err(err(ErrorCode::PsbtValidationFailed, "Outputs exceed inputs"));
        }
        let owned_in: u64 = owned.iter().flatten().map(|o| o.value.to_sat()).sum();
        let back: u64 = psbt
            .unsigned_tx
            .output
            .iter()
            .filter(|o| self.backend.owns_script(id, &o.script_pubkey))
            .map(|o| o.value.to_sat())
            .sum();
        self.grant_for_amount(inner, ctx, "psbt:sign", owned_in.saturating_sub(back))?;
        let fee = in_total
            .map(|t| t - out_total)
            .ok_or_else(|| err(ErrorCode::PolicyViolation, "The fee cannot be determined"))?;
        let vsize = crate::psbt::estimate_signed_vsize(&psbt, &prevouts);
        if fee as u128 * 1000 > self.config.limits.high_fee_rate.0 as u128 * vsize as u128 {
            return Err(err(
                ErrorCode::PolicyViolation,
                "The fee rate exceeds limits.high_fee_rate",
            ));
        }
        if nondefault_sighash {
            return Err(err(ErrorCode::PolicyViolation, "Only SIGHASH_ALL is signed"));
        }
        for &i in &targets {
            psbt.inputs[i]
                .witness_utxo
                .get_or_insert_with(|| owned[i].clone().unwrap());
        }
        let net = chain_network(&account.network).expect("supported network");
        let outputs = psbt
            .unsigned_tx
            .output
            .iter()
            .map(|o| OutputSummary {
                address: bitcoin::Address::from_script(&o.script_pubkey, net)
                    .ok()
                    .map(|a| a.to_string()),
                script_pub_key: o.script_pubkey.to_hex_string(),
                amount_sats: o.value.to_sat(),
                to_this_account: self.backend.owns_script(id, &o.script_pubkey),
            })
            .collect();
        let request = PsbtSignRequest {
            requester: self.requester(inner, ctx.client_id),
            account_id: account.account_id.clone(),
            network: account.network.clone(),
            inputs_to_sign: targets.iter().map(|&i| i as u32).collect(),
            input_count: n,
            outputs,
            fee_sats: fee,
            fee_rate_sat_per_kvb: fee * 1000 / vsize,
            psbt: psbt.serialize(),
        };
        let op_id = self.new_operation(inner, ctx, "psbt.sign", OpState::AwaitingApproval);
        let (reply, answer) = oneshot::channel();
        let client_id = ctx.client_id.clone();
        let decide = move |provider: &Arc<Provider>, inner: &mut Inner, op_id: &Id, yes: bool| {
            let sign = |provider: &Arc<Provider>, inner: &mut Inner| -> Result<Value, PwcError> {
                provider.still_allowed(inner, &client_id, "psbt:sign", &account)?;
                let signed = provider
                    .backend
                    .sign_psbt(account.account_id.as_str(), &mut psbt, &targets)?;
                Ok(json!({
                    "account_id": account.account_id,
                    "network": account.network,
                    "psbt": Base64(psbt.serialize()),
                    "psbt_version": 0,
                    "signed_input_indexes": signed,
                    "signing_status": crate::psbt::signing_status(&psbt),
                }))
            };
            provider.conclude(inner, op_id, yes, sign);
        };
        self.await_approval(
            inner,
            &op_id,
            ctx.client_id,
            Prompt::SignPsbt { request, reply },
            answer,
            decide,
        );
        Ok(Outcome::Operation(op_id))
    }

    fn message_sign(
        self: &Arc<Self>,
        inner: &mut Inner,
        ctx: &Ctx,
        p: crate::methods::MessageSign,
    ) -> Result<Outcome, PwcError> {
        self.account_method(ctx, "message.sign")?;
        let account = ctx.account().clone();
        let address = parse_address(
            p.address.as_str(),
            chain_network(&account.network).expect("supported network"),
        )?;
        format_signs(p.format, &address)?;
        if !self
            .backend
            .owns_script(account.account_id.as_str(), &address.script_pubkey())
        {
            return Err(err(ErrorCode::InvalidArgument, "The account does not own this address"));
        }
        let request = MessageSignRequest {
            requester: self.requester(inner, ctx.client_id),
            account_id: account.account_id.clone(),
            network: account.network.clone(),
            address: p.address.to_string(),
            message: p.message.clone(),
            format: p.format,
        };
        let op_id = self.new_operation(inner, ctx, "message.sign", OpState::AwaitingApproval);
        let (reply, answer) = oneshot::channel();
        let client_id = ctx.client_id.clone();
        let decide = move |provider: &Arc<Provider>, inner: &mut Inner, op_id: &Id, yes: bool| {
            let sign = |provider: &Arc<Provider>, inner: &mut Inner| -> Result<Value, PwcError> {
                provider.still_allowed(inner, &client_id, "messages:sign", &account)?;
                let sig = provider.backend.sign_message(
                    account.account_id.as_str(),
                    &address.script_pubkey(),
                    &p.message,
                    p.format,
                )?;
                Ok(json!({ "signature": Base64(sig), "format": p.format, "address": p.address }))
            };
            provider.conclude(inner, op_id, yes, sign);
        };
        self.await_approval(
            inner,
            &op_id,
            ctx.client_id,
            Prompt::SignMessage { request, reply },
            answer,
            decide,
        );
        Ok(Outcome::Operation(op_id))
    }

    /// After approval the request is authorized afresh (Core §7 step 5).
    fn still_allowed(&self, inner: &Inner, client_id: &Id, scope: &str, account: &AccountInfo) -> Result<(), PwcError> {
        if !self.authorized(inner, client_id, scope, Some(&account.account_id), Timestamp::now()) {
            return Err(err(ErrorCode::PermissionRevoked, "The grant was revoked"));
        }
        let wallet = self
            .backend
            .wallet(account.wallet_id.as_str())
            .ok_or_else(|| err(ErrorCode::StateUnavailable, "The wallet is unavailable"))?;
        super::wallet_readable(&wallet)
    }

    /// Ends an approval Operation: refused, or completed or failed by `run`.
    fn conclude(
        self: &Arc<Self>,
        inner: &mut Inner,
        op_id: &Id,
        approved: bool,
        run: impl FnOnce(&Arc<Provider>, &mut Inner) -> Result<Value, PwcError>,
    ) {
        if !approved {
            let e = err(ErrorCode::UserRejected, "The User refused the request");
            self.transition(inner, op_id, OpState::Rejected, None, Some(e));
            return;
        }
        match run(self, inner) {
            Ok(result) => self.transition(inner, op_id, OpState::Completed, Some(result), None),
            Err(e) => self.fail(inner, op_id, e),
        }
    }

    /// Core §4.2: a method absent from the account's `methods` is unsupported for it.
    fn account_method(&self, ctx: &Ctx, method: &str) -> Result<(), PwcError> {
        if ctx.account().methods.contains(&method) {
            Ok(())
        } else {
            Err(err(
                ErrorCode::UnsupportedOperation,
                format!("{method} is not supported for this account"),
            ))
        }
    }

    fn cursor_mac(&self, inner: &Inner, method: &str, ctx: &Ctx, offset: usize) -> String {
        use bitcoin::hashes::{sha256, Hash, HashEngine, Hmac, HmacEngine};
        let mut e = HmacEngine::<sha256::Hash>::new(&inner.state.cursor_key);
        let account = ctx.account().account_id.as_str();
        e.input(format!("{method}\u{0}{account}\u{0}{}\u{0}{offset}", ctx.client_id).as_bytes());
        let mac = Hmac::<sha256::Hash>::from_engine(e).to_byte_array();
        bitcoin::hex::DisplayHex::to_lower_hex_string(&mac[..16])
    }

    /// Core §8 pagination with cursors bound to the method, account and Client.
    fn page(
        &self,
        inner: &Inner,
        ctx: &Ctx,
        method: &str,
        p: &Listing,
        items: Vec<Value>,
        key: &str,
    ) -> Result<Outcome, PwcError> {
        let max = self.config.limits.max_page_size as usize;
        let offset = match &p.cursor {
            None => 0,
            Some(c) => {
                let bad = || err(ErrorCode::InvalidArgument, "The cursor is not valid for this listing");
                let (n, mac) = c.as_str().split_once('.').ok_or_else(bad)?;
                let n: usize = n.parse().map_err(|_| bad())?;
                let expected = self.cursor_mac(inner, method, ctx, n);
                if mac.len() != expected.len()
                    || !bitcoin::hashes::cmp::fixed_time_eq(expected.as_bytes(), mac.as_bytes())
                {
                    return Err(bad());
                }
                if n > items.len() {
                    return Err(err(ErrorCode::CursorExpired, "The listing changed; restart it"));
                }
                n
            }
        };
        let size = p.page_size.map_or(50.min(max), |s| (s.0 as usize).min(max));
        let end = (offset + size).min(items.len());
        let mut v = json!({ key: items[offset..end] });
        if end < items.len() {
            v["next_cursor"] = json!(format!("{end}.{}", self.cursor_mac(inner, method, ctx, end)));
        }
        Ok(Outcome::AccountData(v))
    }

    fn own_operation(&self, inner: &Inner, client_id: &Id, op_id: &Id) -> Result<OpState, PwcError> {
        match inner.state.operations.get(op_id) {
            Some(o) if &o.client_id == client_id => Ok(o.op.state),
            _ => Err(err(ErrorCode::OperationNotFound, "Operation not found")),
        }
    }

    fn request_permissions(self: &Arc<Self>, inner: &mut Inner, ctx: &Ctx, p: RequestPermissions) -> Outcome {
        let op_id = self.new_operation(inner, ctx, "session.requestPermissions", OpState::AwaitingApproval);
        let (wallet_ids, account_ids) = grants::known_resources(&*self.backend, &p);
        let request = PermissionRequest {
            requester: self.requester(inner, ctx.client_id),
            scopes: p.scopes.clone(),
            network: p.network.clone(),
            wallet_ids,
            account_ids,
            expires_at: p.expires_at,
            constraints: p.constraints.clone(),
        };
        let (reply, answer) = oneshot::channel();
        let client_id = ctx.client_id.clone();
        let decide = move |provider: &Arc<Provider>, inner: &mut Inner, op_id: &Id, choice: Option<GrantChoice>| {
            let now = Timestamp::now();
            match choice.and_then(|c| grants::narrow(&*provider.backend, &p, c, &client_id, now)) {
                Some(grant) => {
                    let result = json!({ "grants": [grant.to_json(now)] });
                    provider.schedule_expiry(&grant);
                    inner.state.grants.insert(grant.grant_id.clone(), grant);
                    provider.transition(inner, op_id, OpState::Completed, Some(result), None);
                }
                None => {
                    let e = err(ErrorCode::UserRejected, "The User refused the request");
                    provider.transition(inner, op_id, OpState::Rejected, None, Some(e));
                }
            }
        };
        self.await_approval(
            inner,
            &op_id,
            ctx.client_id,
            Prompt::Permissions { request, reply },
            answer,
            decide,
        );
        Outcome::Operation(op_id)
    }

    /// At a grant's expiry, unfinished Operations that relied on it fail (Core §7 step 6).
    pub(super) fn schedule_expiry(self: &Arc<Self>, grant: &Grant) {
        let Some(expiry) = grant.expiry() else { return };
        let provider = self.clone();
        let client_id = grant.client_id.clone();
        let grant_id = grant.grant_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(expiry.remaining()).await;
            let mut inner = provider.lock();
            if inner.state.grants.get(&grant_id).is_some_and(|g| !g.revoked) {
                let audience = super::Audience::Client(&client_id);
                provider.emit(
                    &inner,
                    "grant.expired",
                    None,
                    None,
                    json!({ "grant_id": grant_id }),
                    audience,
                );
            }
            provider.reevaluate(&mut inner, &client_id);
        });
    }
}
