//! Core §4.8 grants: coverage, expiry and narrowing of the User's choice.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::backend::{AccountInfo, WalletBackend, WalletInfo};
use crate::methods::RequestPermissions;
use crate::prompt::GrantChoice;
use crate::types::{chain_network, GrantConstraints, Id, Scope, Timestamp};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub grant_id: Id,
    pub client_id: Id,
    pub network: String,
    pub scopes: Vec<String>,
    pub wallet_ids: Vec<Id>,
    pub account_ids: Vec<Id>,
    pub created_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub constraints: Option<GrantConstraints>,
    pub revoked: bool,
}

impl Grant {
    /// The earlier of `expires_at` and `constraints.valid_until`.
    pub fn expiry(&self) -> Option<Timestamp> {
        let until = self.constraints.as_ref().and_then(|c| c.valid_until);
        earlier(self.expires_at, until)
    }

    pub fn status(&self, now: Timestamp) -> &'static str {
        if self.revoked {
            "revoked"
        } else if self.expiry().is_some_and(|e| e <= now) {
            "expired"
        } else {
            "active"
        }
    }

    pub fn active(&self, now: Timestamp) -> bool {
        self.status(now) == "active"
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    pub fn covers_wallet(&self, w: &WalletInfo) -> bool {
        self.network == w.network
            && (self.wallet_ids.contains(&w.wallet_id) || w.account_ids.iter().any(|a| self.account_ids.contains(a)))
    }

    pub fn covers_account(&self, a: &AccountInfo) -> bool {
        self.network == a.network
            && (self.wallet_ids.contains(&a.wallet_id) || self.account_ids.contains(&a.account_id))
    }

    pub fn to_json(&self, now: Timestamp) -> Value {
        let mut v = json!({
            "grant_id": self.grant_id,
            "client_id": self.client_id,
            "network": self.network,
            "scopes": self.scopes,
            "wallet_ids": self.wallet_ids,
            "account_ids": self.account_ids,
            "created_at": self.created_at,
            "status": self.status(now),
        });
        if let Some(e) = self.expires_at {
            v["expires_at"] = json!(e);
        }
        if let Some(c) = &self.constraints {
            v["constraints"] = json!(c);
        }
        v
    }
}

fn earlier(a: Option<Timestamp>, b: Option<Timestamp>) -> Option<Timestamp> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Keeps only identifiers of existing wallets and accounts (Core §8.1: unknown ones are ignored silently).
pub fn known_resources(backend: &dyn WalletBackend, p: &RequestPermissions) -> (Vec<Id>, Vec<Id>) {
    let wallets = p
        .wallet_ids
        .iter()
        .flatten()
        .filter(|w| backend.wallet(w.as_str()).is_some())
        .cloned()
        .collect();
    let accounts = p
        .account_ids
        .iter()
        .flatten()
        .filter(|a| backend.account(a.as_str()).is_some())
        .cloned()
        .collect();
    (wallets, accounts)
}

/// Narrows the User's choice so that the grant never exceeds the request (Core §8.1, test C-52).
/// `None` means the choice grants nothing and the request counts as refused.
pub fn narrow(
    backend: &dyn WalletBackend,
    request: &RequestPermissions,
    choice: GrantChoice,
    client_id: &Id,
    now: Timestamp,
) -> Option<Grant> {
    let network = choice.network.as_str();
    chain_network(network)?;
    if request.network.as_ref().is_some_and(|n| n.as_str() != network) {
        return None;
    }
    let scopes: Vec<String> = request
        .scopes
        .iter()
        .filter(|s| choice.scopes.contains(s) && crate::types::SCOPES.contains(&s.as_str()))
        .map(Scope::to_string)
        .collect();
    if scopes.is_empty() {
        return None;
    }
    let (req_wallets, req_accounts) = known_resources(backend, request);
    let listed = !req_wallets.is_empty() || !req_accounts.is_empty();
    let mut wallet_ids: Vec<Id> = Vec::new();
    for w in choice.wallet_ids {
        let ok = backend.wallet(w.as_str()).is_some_and(|x| x.network == network);
        if ok && (!listed || req_wallets.contains(&w)) && !wallet_ids.contains(&w) {
            wallet_ids.push(w);
        }
    }
    let mut account_ids: Vec<Id> = Vec::new();
    for a in choice.account_ids {
        let Some(info) = backend.account(a.as_str()) else {
            continue;
        };
        let requested = req_accounts.contains(&a) || req_wallets.contains(&info.wallet_id);
        if info.network == network
            && !wallet_ids.contains(&info.wallet_id)
            && (!listed || requested)
            && !account_ids.contains(&a)
        {
            account_ids.push(a);
        }
    }
    let expires_at = match request.expires_at {
        Some(r) => Some(choice.expires_at.map_or(r, |c| c.min(r))),
        None => choice.expires_at,
    };
    let req_c = request.constraints.clone().unwrap_or_default();
    let ch_c = choice.constraints.unwrap_or_default();
    let constraints = GrantConstraints {
        max_amount_sats_per_request: match (req_c.max_amount_sats_per_request, ch_c.max_amount_sats_per_request) {
            (Some(r), Some(c)) => Some(r.min(c)),
            (r, c) => r.or(c),
        },
        valid_until: earlier(req_c.valid_until, ch_c.valid_until),
    };
    let constraints = (constraints != GrantConstraints::default()).then_some(constraints);
    Some(Grant {
        grant_id: Id::random("gr"),
        client_id: client_id.clone(),
        network: network.to_string(),
        scopes,
        wallet_ids,
        account_ids,
        created_at: now,
        expires_at,
        constraints,
        revoked: false,
    })
}
