use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Core §10 error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidArgument,
    PayloadTooLarge,
    UnsupportedMethod,
    UnsupportedOperation,
    UnsupportedPsbtVersion,
    UnsupportedScript,
    Unauthenticated,
    PermissionRequired,
    PermissionRevoked,
    GrantNotFound,
    GrantConstraintViolated,
    UserRejected,
    WalletLocked,
    WalletNotFound,
    AccountNotFound,
    NetworkMismatch,
    NetworkUnavailable,
    SyncIncomplete,
    StateUnavailable,
    SignerUnavailable,
    InsufficientFunds,
    PolicyViolation,
    PsbtValidationFailed,
    PsbtConflict,
    TransactionRejected,
    TransactionNotFound,
    ReservationNotFound,
    OperationNotFound,
    OperationNotCancelable,
    OperationExpired,
    OperationCanceled,
    IdempotencyConflict,
    CursorExpired,
    RateLimited,
    ProviderUnavailable,
    InternalError,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 36] = [
        Self::InvalidArgument,
        Self::PayloadTooLarge,
        Self::UnsupportedMethod,
        Self::UnsupportedOperation,
        Self::UnsupportedPsbtVersion,
        Self::UnsupportedScript,
        Self::Unauthenticated,
        Self::PermissionRequired,
        Self::PermissionRevoked,
        Self::GrantNotFound,
        Self::GrantConstraintViolated,
        Self::UserRejected,
        Self::WalletLocked,
        Self::WalletNotFound,
        Self::AccountNotFound,
        Self::NetworkMismatch,
        Self::NetworkUnavailable,
        Self::SyncIncomplete,
        Self::StateUnavailable,
        Self::SignerUnavailable,
        Self::InsufficientFunds,
        Self::PolicyViolation,
        Self::PsbtValidationFailed,
        Self::PsbtConflict,
        Self::TransactionRejected,
        Self::TransactionNotFound,
        Self::ReservationNotFound,
        Self::OperationNotFound,
        Self::OperationNotCancelable,
        Self::OperationExpired,
        Self::OperationCanceled,
        Self::IdempotencyConflict,
        Self::CursorExpired,
        Self::RateLimited,
        Self::ProviderUnavailable,
        Self::InternalError,
    ];
}

/// Core §10 `PwcError`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PwcError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl PwcError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), details: None, retryable: None }
    }
}

impl std::fmt::Display for PwcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for PwcError {}

/// Shorthand used throughout the crate.
pub fn err(code: ErrorCode, message: impl Into<String>) -> PwcError {
    PwcError::new(code, message)
}
