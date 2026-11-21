//! Core §3.2 semantic types. Each type validates its syntax when deserialized, so a parsed parameter
//! object holds only well-formed values (Core §3.1: syntax errors are `INVALID_ARGUMENT`).

use std::fmt;

use base64::Engine;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const MAX_MONEY: u64 = 2_100_000_000_000_000;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_FEE_RATE: u64 = 1_000_000_000;

macro_rules! string_type {
    ($(#[$doc:meta])* $name:ident, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                let check: fn(&str) -> bool = $check;
                if check(&s) {
                    Ok(Self(s))
                } else {
                    Err(D::Error::custom(concat!("invalid ", stringify!($name))))
                }
            }
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_type!(
    /// Opaque identifier, 1 to 128 characters of `[A-Za-z0-9._~:-]`.
    Id,
    |s| (1..=128).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._~:-".contains(&b))
);
string_type!(
    /// 64 lowercase hex characters.
    Hash256,
    |s| s.len() == 64 && is_lower_hex(s)
);
string_type!(
    /// `[a-z0-9-]{1,32}`.
    Network,
    |s| (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
);
string_type!(
    /// Display text of 1 to 256 characters without control characters.
    Text,
    |s| {
        let n = s.chars().count();
        (1..=256).contains(&n) && !s.chars().any(|c| matches!(c as u32, 0x00..=0x1f | 0x7f..=0x9f))
    }
);
string_type!(
    /// Opaque pagination marker.
    Cursor,
    |s| !s.is_empty()
);
string_type!(
    /// An address string; its encoding is checked against a network by the method that uses it.
    Address,
    |s| !s.is_empty()
);

impl Id {
    /// A fresh random identifier `<prefix>-<32 hex>`.
    pub fn random(prefix: &str) -> Id {
        Id(format!(
            "{prefix}-{}",
            bitcoin::hex::DisplayHex::to_lower_hex_string(&random_bytes::<16>()[..])
        ))
    }
}

impl Text {
    /// Builds display text from a trusted constant or truncates and cleans arbitrary text.
    pub fn lossy(s: &str) -> Text {
        let cleaned: String = s
            .chars()
            .filter(|c| !matches!(*c as u32, 0x00..=0x1f | 0x7f..=0x9f))
            .take(256)
            .collect();
        Text(if cleaned.is_empty() { "?".into() } else { cleaned })
    }
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("operating-system random source");
    b
}

fn parse_canonical(s: &str) -> Option<u64> {
    let canonical =
        s == "0" || (s.starts_with(|c: char| c.is_ascii_digit() && c != '0') && s.bytes().all(|b| b.is_ascii_digit()));
    if canonical && s.len() <= 19 {
        s.parse().ok()
    } else {
        None
    }
}

macro_rules! amount_type {
    ($(#[$doc:meta])* $name:ident, $min:expr, $max:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u64);

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                match parse_canonical(&s) {
                    Some(v) if ($min..=$max).contains(&v) => Ok(Self(v)),
                    _ => Err(D::Error::custom(concat!("invalid ", stringify!($name)))),
                }
            }
        }
    };
}

amount_type!(
    /// Canonical nonnegative satoshi amount.
    Sats, 0, MAX_MONEY
);
amount_type!(
    /// Canonical positive satoshi amount.
    PositiveSats, 1, MAX_MONEY
);
amount_type!(
    /// Fee rate in sat/kvB.
    FeeRate, 1, MAX_FEE_RATE
);

/// Signed satoshi delta, serialized as a decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedSats(pub i64);

impl Serialize for SignedSats {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for SignedSats {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let (neg, digits) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s.as_str()),
        };
        match parse_canonical(digits) {
            Some(v) if v <= MAX_MONEY && !(neg && v == 0) => Ok(Self(if neg { -(v as i64) } else { v as i64 })),
            _ => Err(D::Error::custom("invalid SignedSats")),
        }
    }
}

/// JSON integer 1 through 2^53 − 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct PositiveInteger(pub u64);

impl<'de> Deserialize<'de> for PositiveInteger {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match u64::deserialize(d)? {
            v @ 1..=MAX_SAFE_INTEGER => Ok(Self(v)),
            _ => Err(D::Error::custom("invalid PositiveInteger")),
        }
    }
}

/// PSBT version; only 0 is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PsbtVersion;

impl Serialize for PsbtVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(0)
    }
}

impl<'de> Deserialize<'de> for PsbtVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match u8::deserialize(d)? {
            0 => Ok(Self),
            _ => Err(D::Error::custom("invalid PsbtVersion")),
        }
    }
}

/// Non-empty, even-length lowercase hex, holding the decoded bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hex(pub Vec<u8>);

impl Serialize for Hex {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&bitcoin::hex::DisplayHex::to_lower_hex_string(&self.0[..]))
    }
}

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() || s.len() % 2 != 0 || !is_lower_hex(&s) {
            return Err(D::Error::custom("invalid Hex"));
        }
        bitcoin::hex::FromHex::from_hex(&s)
            .map(Hex)
            .map_err(|_| D::Error::custom("invalid Hex"))
    }
}

/// Non-empty canonical padded base64, holding the decoded bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base64(pub Vec<u8>);

impl Serialize for Base64 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Base64 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match base64::engine::general_purpose::STANDARD.decode(&s) {
            Ok(b) if !b.is_empty() => Ok(Base64(b)),
            _ => Err(D::Error::custom("invalid Base64")),
        }
    }
}

/// RFC 3339 UTC timestamp with the `Z` suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    pub fn now() -> Timestamp {
        let t = OffsetDateTime::now_utc();
        Timestamp(t.replace_nanosecond(t.millisecond() as u32 * 1_000_000).unwrap())
    }

    pub fn after(self, d: std::time::Duration) -> Timestamp {
        Timestamp(self.0 + d)
    }

    /// Time from now until `self`, zero if it has passed.
    pub fn remaining(self) -> std::time::Duration {
        (self.0 - OffsetDateTime::now_utc()).try_into().unwrap_or_default()
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let f =
            time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
        s.serialize_str(&self.0.format(f).map_err(serde::ser::Error::custom)?)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let b = s.as_bytes();
        let shape = b.len() >= 20
            && b.last() == Some(&b'Z')
            && b[4] == b'-'
            && b[7] == b'-'
            && b[10] == b'T'
            && b[13] == b':'
            && b[16] == b':'
            && (b.len() == 20 || (b[19] == b'.' && b.len() > 21 && b[20..b.len() - 1].iter().all(u8::is_ascii_digit)));
        match OffsetDateTime::parse(&s, &Rfc3339) {
            Ok(t) if shape => Ok(Timestamp(t)),
            _ => Err(D::Error::custom("invalid Timestamp")),
        }
    }
}

/// Core §3.4: `ceil(rate * vsize / 1000)` in exact integer arithmetic.
pub fn min_fee(sat_per_kvb: u64, vsize: u64) -> u64 {
    let product = sat_per_kvb as u128 * vsize as u128;
    product.div_ceil(1000) as u64
}

fn vendor_name(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("vendor.") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('.').collect();
    let (last, domain) = parts.split_last().unwrap();
    !domain.is_empty()
        && domain.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        && last.starts_with(|c: char| c.is_ascii_alphabetic())
        && last
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Core §6 scopes this Provider can grant.
pub const SCOPES: [&str; 11] = [
    "wallets:read",
    "balances:read",
    "addresses:read",
    "addresses:create",
    "transactions:read",
    "utxos:read",
    "psbt:create",
    "psbt:sign",
    "transactions:broadcast",
    "messages:sign",
    "events:read",
];

string_type!(
    /// A Core scope or a vendor extension scope.
    Scope,
    |s| SCOPES.contains(&s) || vendor_name(s)
);

/// Networks this Provider can handle. Purity uses the Bitcoin Core address formats.
pub const SUPPORTED_NETWORKS: [&str; 3] = ["mainnet", "signet", "regtest"];

/// The address-encoding network of a supported PWC network name.
pub fn chain_network(network: &str) -> Option<bitcoin::Network> {
    match network {
        "mainnet" => Some(bitcoin::Network::Bitcoin),
        "signet" => Some(bitcoin::Network::Signet),
        "regtest" => Some(bitcoin::Network::Regtest),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptType {
    P2wpkh,
    P2pkh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageFormat {
    #[serde(rename = "bip322-simple")]
    Bip322Simple,
    #[serde(rename = "bip322-full")]
    Bip322Full,
    #[serde(rename = "legacy")]
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutPoint {
    pub txid: Hash256,
    pub vout: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeePolicy {
    Rate { sat_per_kvb: FeeRate },
    WalletEstimate { target_blocks: PositiveInteger },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantConstraints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount_sats_per_request: Option<PositiveSats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<Timestamp>,
}
