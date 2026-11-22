//! Backend data types: the Rust records of `pwc_provider::backend` and their Java records.

/// The values of the `&'static str` fields, as in the strict schema.
pub const SYNC_STATES: [&str; 4] = ["synced", "syncing", "offline", "unknown"];
pub const ADDRESS_ROLES: [&str; 3] = ["receive", "change", "unknown"];
pub const ADDRESS_STATES: [&str; 3] = ["unused", "used", "unknown"];
pub const UTXO_RESTRICTIONS: [&str; 5] = ["frozen", "locked", "immature", "unconfirmed", "unknown"];
pub const TRANSACTION_STATUSES: [&str; 6] = ["unconfirmed", "confirmed", "conflicted", "replaced", "dropped", "unknown"];
