//! Requests for a User decision, delivered to the host wallet's trusted UI.
//!
//! Each `Prompt` carries a oneshot reply. Dropping the reply refuses the request. The Provider withdraws a
//! prompt by dropping its receiving end, which the UI can observe with `reply.is_closed()`.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::types::{GrantConstraints, Id, Network, Scope, Text, Timestamp};

/// Operating-system information about a connected process (LOCAL §3.4), as reported by the OS.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pid: Option<i32>,
    pub executable: Option<PathBuf>,
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.pid {
            Some(pid) => write!(f, "process {pid}")?,
            None => write!(f, "unknown process")?,
        }
        if let Some(exe) = &self.executable {
            write!(f, ", executable {}", exe.display())?;
        }
        write!(f, " (reported by the operating system)")
    }
}

/// LOCAL §5.3 pairing prompt.
#[derive(Debug, Clone)]
pub struct PairingRequest {
    pub claimed_name: Text,
    pub claimed_description: Option<Text>,
    pub pairing_code: String,
    pub peer: PeerInfo,
}

impl fmt::Display for PairingRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Pairing request from an application calling itself \"{}\" (claimed by the application, unverified)",
            self.claimed_name
        )?;
        if let Some(d) = &self.claimed_description {
            writeln!(f, "  description (unverified): {d}")?;
        }
        writeln!(f, "  peer: {}", self.peer)?;
        write!(
            f,
            "  pairing code: {} (compare it with the code the application shows)",
            self.pairing_code
        )
    }
}

/// The requesting Client as recorded at pairing; its name is a claim.
#[derive(Debug, Clone)]
pub struct Requester {
    pub client_id: Id,
    pub claimed_name: Text,
    pub peer: PeerInfo,
}

impl fmt::Display for Requester {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" (claimed by the application, unverified; {}; {})",
            self.claimed_name, self.client_id, self.peer
        )
    }
}

/// Core §8.1 permission request. Unknown wallet and account identifiers have been removed.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    pub requester: Requester,
    pub scopes: Vec<Scope>,
    pub network: Option<Network>,
    pub wallet_ids: Vec<Id>,
    pub account_ids: Vec<Id>,
    pub expires_at: Option<Timestamp>,
    pub constraints: Option<GrantConstraints>,
}

impl fmt::Display for PermissionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Permission request from {}", self.requester)?;
        let scopes: Vec<&str> = self.scopes.iter().map(|s| s.as_str()).collect();
        writeln!(f, "  scopes: {}", scopes.join(", "))?;
        if self.scopes.iter().any(|s| s.as_str() == "psbt:create") {
            writeln!(
                f,
                "  note: PSBTs it creates disclose the selected coins and their key-origin data"
            )?;
        }
        if let Some(n) = &self.network {
            writeln!(f, "  network: {n}")?;
        }
        if !self.wallet_ids.is_empty() || !self.account_ids.is_empty() {
            writeln!(
                f,
                "  requested wallets: {:?}, accounts: {:?}",
                self.wallet_ids, self.account_ids
            )?;
        }
        if let Some(t) = &self.expires_at {
            writeln!(f, "  requested expiry: {}", serde_json::to_string(t).unwrap())?;
        }
        if let Some(c) = &self.constraints {
            writeln!(f, "  requested constraints: {}", serde_json::to_string(c).unwrap())?;
        }
        Ok(())
    }
}

/// The User's decision on a permission request. The Provider narrows it to the request (Core §8.1).
#[derive(Debug, Clone)]
pub struct GrantChoice {
    pub network: Network,
    pub scopes: Vec<Scope>,
    pub wallet_ids: Vec<Id>,
    pub account_ids: Vec<Id>,
    pub expires_at: Option<Timestamp>,
    pub constraints: Option<GrantConstraints>,
}

/// One output of a transaction to sign, derived from the PSBT itself (Core §9.2).
#[derive(Debug, Clone)]
pub struct OutputSummary {
    /// The address, when the script has an address form on the account's network.
    pub address: Option<String>,
    pub script_pub_key: String,
    pub amount_sats: u64,
    pub to_this_account: bool,
}

/// A `psbt.sign` request; every field is computed from the PSBT and the wallet, not from the Client.
#[derive(Debug, Clone)]
pub struct PsbtSignRequest {
    pub requester: Requester,
    pub account_id: Id,
    pub network: String,
    /// The inputs the account will sign.
    pub inputs_to_sign: Vec<u32>,
    pub input_count: usize,
    pub outputs: Vec<OutputSummary>,
    pub fee_sats: u64,
    pub fee_rate_sat_per_kvb: u64,
    /// The exact PSBT the approval binds to.
    pub psbt: Vec<u8>,
}

impl fmt::Display for PsbtSignRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Transaction signing request from {}", self.requester)?;
        writeln!(f, "  account {} on {}", self.account_id, self.network)?;
        writeln!(f, "  sign inputs {:?} of {}", self.inputs_to_sign, self.input_count)?;
        for o in &self.outputs {
            let dest = o
                .address
                .clone()
                .unwrap_or_else(|| format!("script {}", o.script_pub_key));
            let own = if o.to_this_account {
                " (back to this account)"
            } else {
                ""
            };
            writeln!(f, "  pay {} sat to {dest}{own}", o.amount_sats)?;
        }
        write!(f, "  fee {} sat ({} sat/kvB)", self.fee_sats, self.fee_rate_sat_per_kvb)
    }
}

/// A `message.sign` request with the exact message.
#[derive(Debug, Clone)]
pub struct MessageSignRequest {
    pub requester: Requester,
    pub account_id: Id,
    pub network: String,
    pub address: String,
    pub message: String,
    pub format: crate::types::MessageFormat,
}

impl fmt::Display for MessageSignRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Message signing request from {}", self.requester)?;
        writeln!(
            f,
            "  address {} ({}) on {}",
            self.address,
            serde_json::to_string(&self.format).unwrap(),
            self.network
        )?;
        write!(
            f,
            "  message (exact bytes, JSON-escaped): {}",
            serde_json::to_string(&self.message).unwrap()
        )
    }
}

pub enum Prompt {
    Pairing {
        request: PairingRequest,
        reply: oneshot::Sender<bool>,
    },
    Permissions {
        request: PermissionRequest,
        reply: oneshot::Sender<Option<GrantChoice>>,
    },
    SignPsbt {
        request: PsbtSignRequest,
        reply: oneshot::Sender<bool>,
    },
    SignMessage {
        request: MessageSignRequest,
        reply: oneshot::Sender<bool>,
    },
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prompt::Pairing { request, .. } => request.fmt(f),
            Prompt::Permissions { request, .. } => request.fmt(f),
            Prompt::SignPsbt { request, .. } => request.fmt(f),
            Prompt::SignMessage { request, .. } => request.fmt(f),
        }
    }
}
