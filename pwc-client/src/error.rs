use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

macro_rules! error_codes {
    ($($variant:ident => $name:literal),* $(,)?) => {
        /// The error codes of Core §10. A code this version does not define, such as a `vendor.*` extension, is `Other`.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
        pub enum ErrorCode {
            $($variant,)*
            Other(String),
        }

        impl ErrorCode {
            pub fn as_str(&self) -> &str {
                match self {
                    $(ErrorCode::$variant => $name,)*
                    ErrorCode::Other(s) => s,
                }
            }
        }

        impl From<&str> for ErrorCode {
            fn from(s: &str) -> ErrorCode {
                match s {
                    $($name => ErrorCode::$variant,)*
                    _ => ErrorCode::Other(s.to_string()),
                }
            }
        }
    };
}

error_codes! {
    InvalidArgument => "INVALID_ARGUMENT",
    PayloadTooLarge => "PAYLOAD_TOO_LARGE",
    UnsupportedMethod => "UNSUPPORTED_METHOD",
    UnsupportedOperation => "UNSUPPORTED_OPERATION",
    UnsupportedPsbtVersion => "UNSUPPORTED_PSBT_VERSION",
    UnsupportedScript => "UNSUPPORTED_SCRIPT",
    Unauthenticated => "UNAUTHENTICATED",
    PermissionRequired => "PERMISSION_REQUIRED",
    PermissionRevoked => "PERMISSION_REVOKED",
    GrantNotFound => "GRANT_NOT_FOUND",
    GrantConstraintViolated => "GRANT_CONSTRAINT_VIOLATED",
    UserRejected => "USER_REJECTED",
    WalletLocked => "WALLET_LOCKED",
    WalletNotFound => "WALLET_NOT_FOUND",
    AccountNotFound => "ACCOUNT_NOT_FOUND",
    NetworkMismatch => "NETWORK_MISMATCH",
    NetworkUnavailable => "NETWORK_UNAVAILABLE",
    SyncIncomplete => "SYNC_INCOMPLETE",
    StateUnavailable => "STATE_UNAVAILABLE",
    SignerUnavailable => "SIGNER_UNAVAILABLE",
    InsufficientFunds => "INSUFFICIENT_FUNDS",
    PolicyViolation => "POLICY_VIOLATION",
    PsbtValidationFailed => "PSBT_VALIDATION_FAILED",
    PsbtConflict => "PSBT_CONFLICT",
    TransactionRejected => "TRANSACTION_REJECTED",
    TransactionNotFound => "TRANSACTION_NOT_FOUND",
    ReservationNotFound => "RESERVATION_NOT_FOUND",
    OperationNotFound => "OPERATION_NOT_FOUND",
    OperationNotCancelable => "OPERATION_NOT_CANCELABLE",
    OperationExpired => "OPERATION_EXPIRED",
    OperationCanceled => "OPERATION_CANCELED",
    IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
    CursorExpired => "CURSOR_EXPIRED",
    RateLimited => "RATE_LIMITED",
    ProviderUnavailable => "PROVIDER_UNAVAILABLE",
    InternalError => "INTERNAL_ERROR",
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ErrorCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<ErrorCode, D::Error> {
        Ok(ErrorCode::from(String::deserialize(d)?.as_str()))
    }
}

/// The `PwcError` of Core §10. Branch on `code`, never on `message`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct PwcError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
pub enum Error {
    /// An error from the Provider: the answer to a request, or the `error` of a terminal Operation.
    Pwc(PwcError),
    /// A JSON-RPC error without a `PwcError`: `-32700` or `-32600` (LOCAL §4.3).
    Rpc {
        code: i64,
        message: String,
    },
    /// The endpoint directory is missing, belongs to another user, or is open to group or others (LOCAL §3.2).
    UnsafeEndpoint(String),
    /// The credential store holds no credential for this Provider.
    NotPaired,
    /// The Provider's `server_proof` was wrong; the connection was closed (LOCAL §5.4).
    ProviderAuthFailed,
    /// The connection is closed. Connect again and retry with the same idempotency key (LOCAL §5.6).
    ConnectionClosed,
    /// The request is longer than the Provider's `max_frame_bytes`.
    FrameTooLarge,
    /// The Provider sent something this version of the protocol does not allow.
    Protocol(String),
    Io(String),
    /// `wait_operation` reached its time limit before the Operation was terminal.
    Timeout,
    /// The Operation is not yet terminal, so it has no result.
    OperationPending,
    /// The Operation completed, but the Provider withholds its result (Core §4.8).
    ResultUnavailable,
    /// The event stream fell behind and events were discarded; query the authoritative state again (LOCAL §8).
    EventsMissed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Pwc(e) => write!(f, "{}: {}", e.code.as_str(), e.message),
            Error::Rpc { code, message } => write!(f, "JSON-RPC error {code}: {message}"),
            Error::UnsafeEndpoint(m) => write!(f, "unsafe endpoint: {m}"),
            Error::NotPaired => f.write_str("no stored credential for this Provider"),
            Error::ProviderAuthFailed => f.write_str("the Provider failed authentication"),
            Error::ConnectionClosed => f.write_str("connection closed"),
            Error::FrameTooLarge => f.write_str("request exceeds max_frame_bytes"),
            Error::Protocol(m) => write!(f, "protocol error: {m}"),
            Error::Io(m) => write!(f, "I/O error: {m}"),
            Error::Timeout => f.write_str("timed out"),
            Error::OperationPending => f.write_str("the Operation is not terminal"),
            Error::ResultUnavailable => f.write_str("the Operation's result is withheld"),
            Error::EventsMissed => f.write_str("events were missed"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e.to_string())
    }
}
