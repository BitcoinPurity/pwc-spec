//! Core §9 Operations.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::PwcError;
use crate::types::{Id, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpState {
    Queued,
    AwaitingApproval,
    AwaitingUnlock,
    AwaitingSigner,
    Processing,
    Completed,
    Rejected,
    Expired,
    Canceled,
    Failed,
}

impl OpState {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Rejected | Self::Expired | Self::Canceled | Self::Failed
        )
    }
}

/// The Core §4.8 `Operation` object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub operation_id: Id,
    pub method: String,
    pub state: OpState,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<PwcError>,
}

/// An Operation with the bookkeeping the Provider needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpRecord {
    pub op: Operation,
    pub client_id: Id,
    /// Scope the Operation's method requires; its result is shown only while the Client holds it.
    pub scope: Option<String>,
    pub wallet_id: Option<Id>,
    pub finished_at: Option<Timestamp>,
}

/// What an idempotency record answers on an exact retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Replay {
    Operation(Id),
    /// A synchronous result; `wallet_id` is set when it contains that wallet's account data.
    Value {
        result: Value,
        wallet_id: Option<Id>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdemRecord {
    pub params: Value,
    pub replay: Replay,
    pub created_at: Timestamp,
}

/// A PSBT input reservation (Core §8.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub client_id: Id,
    pub account_id: Id,
    pub grant_id: Id,
    pub outpoints: Vec<bitcoin::OutPoint>,
    pub until: Timestamp,
    pub released: bool,
}
