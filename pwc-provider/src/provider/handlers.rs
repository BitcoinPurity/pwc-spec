//! Check 10 of Core §8: method-specific checks and execution.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::grants::{self, Grant};
use super::{Inner, OpState, Outcome, Provider, Target};
use crate::backend::{AccountInfo, WalletInfo};
use crate::error::{err, ErrorCode, PwcError};
use crate::methods::{Request, RequestPermissions};
use crate::prompt::{GrantChoice, PermissionRequest, Prompt};
use crate::types::{Id, Timestamp};

pub(super) struct Ctx<'a> {
    pub client_id: &'a Id,
    pub session_id: &'a Id,
    pub now: Timestamp,
    pub target: Option<&'a Target>,
}

impl Ctx<'_> {
    fn account(&self) -> &AccountInfo {
        self.target.and_then(|t| t.account.as_ref()).expect("account resolved at check 7")
    }

    fn wallet(&self) -> &WalletInfo {
        &self.target.expect("wallet resolved at check 7").wallet
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
    pub(super) fn execute(self: &Arc<Self>, inner: &mut Inner, ctx: &Ctx, request: Request) -> Result<Outcome, PwcError> {
        match request {
            Request::GetCapabilities => Ok(Outcome::Value(self.capabilities())),
            Request::GetInfo => {
                let grant_ids: Vec<&Id> = self.active_grants(inner, ctx.client_id, ctx.now).map(|g| &g.grant_id).collect();
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
                let owned = inner.state.grants.get(p.grant_id.as_str()).is_some_and(|g| &g.client_id == ctx.client_id);
                if !owned {
                    return Err(err(ErrorCode::GrantNotFound, "Grant not found"));
                }
                let revoked = self.revoke(inner, p.grant_id.as_str());
                Ok(Outcome::Value(json!({ "revoked": revoked })))
            }
            Request::WalletList => {
                let grants: Vec<&Grant> =
                    self.active_grants(inner, ctx.client_id, ctx.now).filter(|g| g.has_scope("wallets:read")).collect();
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
                            .filter(|a| readable && self.backend.account(a.as_str()).is_some_and(|a| grants.iter().any(|g| g.covers_account(&a))))
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
                        self.active_grants(inner, ctx.client_id, ctx.now).any(|g| g.has_scope("wallets:read") && g.covers_account(a))
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
                    return Err(err(ErrorCode::OperationNotCancelable, "The Operation is past an irreversible step"));
                }
                let e = err(ErrorCode::OperationCanceled, "The Operation was canceled");
                self.transition(inner, &p.operation_id, OpState::Canceled, None, Some(e));
                Ok(Outcome::Operation(p.operation_id))
            }
            _ => {
                let _ = ctx.account();
                Err(err(ErrorCode::InternalError, "not implemented"))
            }
        }
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
        self.await_approval(inner, &op_id, ctx.client_id, Prompt::Permissions { request, reply }, answer, decide);
        Outcome::Operation(op_id)
    }

    /// At a grant's expiry, unfinished Operations that relied on it fail (Core §7 step 6).
    fn schedule_expiry(self: &Arc<Self>, grant: &Grant) {
        let Some(expiry) = grant.expiry() else { return };
        let provider = self.clone();
        let client_id = grant.client_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(expiry.remaining()).await;
            let mut inner = provider.lock();
            provider.reevaluate(&mut inner, &client_id);
        });
    }
}
