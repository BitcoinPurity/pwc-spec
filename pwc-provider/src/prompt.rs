//! Requests for a User decision, delivered to the host wallet's trusted UI.
//!
//! Each `Prompt` carries a oneshot reply. Dropping the reply refuses the request. The Provider withdraws a
//! prompt by dropping its receiving end, which the UI can observe with `reply.is_closed()`.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::types::Text;

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

pub enum Prompt {
    Pairing { request: PairingRequest, reply: oneshot::Sender<bool> },
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prompt::Pairing { request, .. } => request.fmt(f),
        }
    }
}
