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
        writeln!(f, "Pairing request from an application calling itself \"{}\" (claimed by the application, unverified)", self.claimed_name)?;
        if let Some(d) = &self.claimed_description {
            writeln!(f, "  description (unverified): {d}")?;
        }
        writeln!(f, "  peer: {}", self.peer)?;
        write!(f, "  pairing code: {} (compare it with the code the application shows)", self.pairing_code)
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
        write!(f, "\"{}\" (claimed by the application, unverified; {}; {})", self.claimed_name, self.client_id, self.peer)
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
            writeln!(f, "  note: PSBTs it creates disclose the selected coins and their key-origin data")?;
        }
        if let Some(n) = &self.network {
            writeln!(f, "  network: {n}")?;
        }
        if !self.wallet_ids.is_empty() || !self.account_ids.is_empty() {
            writeln!(f, "  requested wallets: {:?}, accounts: {:?}", self.wallet_ids, self.account_ids)?;
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

pub enum Prompt {
    Pairing { request: PairingRequest, reply: oneshot::Sender<bool> },
    Permissions { request: PermissionRequest, reply: oneshot::Sender<Option<GrantChoice>> },
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prompt::Pairing { request, .. } => request.fmt(f),
            Prompt::Permissions { request, .. } => request.fmt(f),
        }
    }
}
