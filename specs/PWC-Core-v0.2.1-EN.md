# Purity Wallet Connect (PWC)
## Core Data Model, Business API and Authorization Semantics

**Specification:** PWC-CORE-001  
**Version:** 0.2.1 (review draft; not a deployed standard)  
**Date:** 2026-11-14  
**Language:** English (normative draft)  
**License:** CC BY 4.0  
**Applies to:** conforming wallet Providers and Clients on desktop, mobile, browser and other platforms  
**Status:** Proposed specification, subject to owner review and security audit

> **Scope rule:** This document defines *what* wallet operations mean, which data is exchanged, and which user approvals are required. It does **not** standardize *how* a message is discovered, paired, authenticated, transported or delivered on a particular platform. Those mechanisms are defined by the PWC transport profiles (§15). The platform-independent session, consent and authorization semantics are included here because every transport has to uphold them.

## 0. Decisions, status and interpretation

### 0.1 Established project requirements

1. The specification family is named **Purity Wallet Connect (PWC)**.
2. The cross-platform data model, business methods, permission semantics and operation lifecycle are defined *before* any platform-specific communication API.
3. Existing Bitcoin Purity network identities, chain parameters, address formats, transaction encodings and `BTC`/satoshi monetary units remain unchanged. PWC assigns no replacement chain identity or consensus rules.
4. The core interface is transport-independent, suitable for desktop, mobile, browser extension and other wallet Provider environments.
5. Wallet-held spending keys remain under wallet/user control. Clients receive only approved disclosures and cryptographically valid outputs.
6. PWC Core concerns **native on-chain wallet operations only**. It does not define Lightning, token/asset protocols, swap/exchange protocols or other application-layer business logic.

### 0.2 Decisions adopted in v0.2.0

- Core request and response values use JSON with named-object parameters. Transport profiles later define framing, connection establishment and wire-level authentication.
- Wallets and accounts are distinct API resources; a Provider without a native account concept exposes an appropriate default account.
- `Sats` and signed satoshi deltas are canonical decimal strings. **`FeeRate` is a positive integer decimal string measured in sat/kvB**, where `kvB` means 1,000 virtual bytes.
- Resource models and method parameters refer to the named semantic types in §3.2 rather than interchangeable plain `string` or `number` declarations.
- `psbt.sign` may add any valid subset of approved signatures and returns an updated PSBT; completing signatures does not itself finalize or broadcast a transaction.
- `psbt.combine` and `psbt.finalize` are account-independent, synchronous, side-effect-free PSBT data operations. `psbt.combine` never finalizes a PSBT.
- Balance snapshots distinguish remaining unconfirmed account outputs, confirmed outpoints consumed by the active pending projection, and the signed net unconfirmed change, with the rules in §4.3.
- Signing is separately approved for each transaction or message. No unattended signing or key-export mechanism is defined.
- Concrete local, browser, mobile and remote communication profiles are deferred until this Core specification is stabilized.

### 0.3 Decisions adopted in v0.2.1

- A broadcast refused by the backend ends its Operation in state `failed` with `TRANSACTION_REJECTED`; the broadcast result carries no `accepted` flag (§8.3).
- Only PSBT version 0 (BIP-174) is supported; a PSBT of any other version is rejected with `UNSUPPORTED_PSBT_VERSION` (§4.7).
- A grant is bound to one network; it expires at the **earlier** of `expires_at` and `constraints.valid_until`; `max_amount_sats_per_request` measures the account's net outflow **including the miner fee**; listing a wallet in `wallet_ids` covers every account of that wallet (§4.8).
- Remaining unconfirmed outputs are reported in two fields, `pending_self_sats` and `pending_external_sats` (§4.3).
- While a wallet is locked, its account data cannot be read (§8).
- PSBT input reservations can be released early with `psbt.releaseReservation` and never block the User's own spending (§8.3).
- The error-code assignments of §10, including the new codes `NETWORK_MISMATCH`, `PAYLOAD_TOO_LARGE`, `GRANT_CONSTRAINT_VIOLATED`, `GRANT_NOT_FOUND`, `RESERVATION_NOT_FOUND`, `TRANSACTION_NOT_FOUND`, `OPERATION_CANCELED` and the transport-level `PROVIDER_UNAVAILABLE`.

The present version remains a **review draft**. Items that still require separate ratification are listed in Appendix A. Published fields and methods below are design requirements, **not** assertions of current software support.

### 0.4 Normative vocabulary

The words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT** and **MAY** have the meanings defined in RFC 2119 and RFC 8174 when, and only when, they appear in capitals. Statements marked *Informative* are non-normative. A transport profile MUST NOT weaken a PWC Core security requirement.

## 1. Purpose and scope

### 1.1 Goals

PWC Core provides interoperable semantics for: capability negotiation; wallet and account discovery after consent; address generation; balance, UTXO and history access; PSBT creation, partial signing, combining and finalization; optional raw transaction broadcasting; operation and approval tracking; event semantics; and permission lifecycle management. It supports single-signature, watch-only, hardware-backed and multisignature wallets without assuming that every implementation can perform every operation.

### 1.2 Non-goals

PWC Core does not define: HTTP paths, ports, local sockets, WebSocket frames, Nostr event kinds, WalletConnect topics, QR/URI formats, mobile callbacks, browser-injection names, push services, key exchange algorithms or transport encryption. It does not manage full nodes, mining, block consensus, indexes or wallet seed backups. It does not guarantee online availability, confirmations, transaction acceptance, network privacy, or wallet continuity across devices. Lightning Network functionality, asset/token protocols, swap/exchange protocols and other application-layer services are outside PWC Core. This specification does not define their interfaces or extensions.

### 1.3 Layering

| Layer | Responsibility | This document |
|---|---|---|
| PWC Core | Data types, methods, authorization, operation lifecycle, errors, capabilities, event definitions | **Normative** |
| PWC Transport Profile | Discovery, peer identity, pairing proof, authenticated channel, request framing, event delivery, reconnection | **Deferred** |
| Wallet implementation | Accounts, coin selection, PSBT handling, user confirmation, keys, hardware devices, network backend | Implementation-specific, subject to Core requirements |
| Chain/wallet standards | Existing Purity network settings, Bitcoin-compatible addresses and PSBT serialization | Reused, **not changed** |

## 2. Actors and trust boundaries

- **User:** Human controlling grants and sensitive approvals.
- **Wallet:** Trusted local or remote application with the authority to inspect its holdings and possibly sign transactions.
- **Provider:** The PWC endpoint of a Wallet: the component that implements the methods of this specification and enforces its requirements. Every normative requirement placed on "the wallet" in this document binds the Provider.
- **Client:** Third-party application that requests access to specific wallet features.
- **Transport:** Carries requests and results; it is *not* a source of permission or spend authority by itself.
- **Session:** Authenticated association between one Provider and one Client principal under one transport binding.
- **Grant:** Revocable authorization, bound to one network, of scopes, wallets, accounts, expiration and optional constraints.
- **Operation:** Durable or recoverable record of a potentially asynchronous action, particularly one needing user confirmation.

A Client-supplied application name, icon or website URL is display metadata, **not** proof of identity. Transport profiles MUST authenticate the Client principal and bind it to the grant. The Provider is authoritative for balance, chain state, eligible UTXOs, policy and approval results; Client-provided labels and transaction summaries are untrusted.

## 3. General data rules

### 3.1 JSON and naming

- JSON data MUST conform to RFC 8259 and UTF-8. Object member names are case-sensitive; core members use `snake_case`. Core method names have the form `category.action`, where `category` is lowercase and `action` is lowerCamelCase, exactly as registered below (for example `account.getBalance`).
- Method inputs use a named-object `params`; success returns a `result` object; failure returns a `PwcError`. A transport profile determines the outer request/response envelope.
- A request that omits a required parameter, or supplies a value of the wrong JSON type or outside the syntax of its semantic type, MUST produce `INVALID_ARGUMENT`.
- Unknown input members MUST NOT be interpreted as authorization and MUST NOT change transaction semantics; a Provider MUST either ignore them or reject the request with `INVALID_ARGUMENT`. Clients MUST ignore unknown response members.
- Unless specified otherwise, optional fields are omitted rather than encoded as `null`. No field defined in this version permits `null`.
- A Provider MUST NOT emit floating-point monetary amounts or use binary floating-point arithmetic for consensus-significant values.
- Arrays retain order. Ordering of object members has no semantic effect. Prototypes, object references and executable expressions are not part of the data model.

### 3.2 Primitive and reusable semantic types

The names in this table are **normative semantic types**, not language-specific aliases. JSON Schema provides their machine-readable constraints. SDKs MUST preserve the semantics when representing values in any programming language.

| Type | JSON representation | Rules |
|---|---|---|
| `Id` | string | Opaque, case-sensitive, 1 to 128 characters from `[A-Za-z0-9._~:-]`; identifier, not secret or authorization proof |
| `Sats` | string | Canonical nonnegative integer, `0` or `[1-9][0-9]*`, at most `2100000000000000` |
| `PositiveSats` | string | Canonical strictly positive integer, `[1-9][0-9]*`, at most `2100000000000000` |
| `SignedSats` | string | `0`, canonical positive integer, or `-` followed by a canonical positive integer; no `-0`, no `+`; magnitude at most `2100000000000000` |
| `FeeRate` | string | **Canonical strictly positive integer in sat/kvB**, `[1-9][0-9]*`, at most `1000000000`, e.g. `"1250"` |
| `UInt32` | JSON integer | 0 through 4,294,967,295 |
| `NonNegativeInteger` | JSON integer | 0 through 9,007,199,254,740,991 (2^53 − 1) |
| `PositiveInteger` | JSON integer | 1 through 9,007,199,254,740,991 (2^53 − 1) |
| `BlockHeight` | JSON integer | Chain height, 0 through 4,294,967,295 |
| `PsbtVersion` | JSON integer | `0` |
| `Timestamp` | string | RFC 3339 in UTC with the `Z` suffix; fractional seconds optional, e.g. `2026-11-14T00:00:00Z` |
| `Hash256` | string | 64 lowercase hexadecimal characters in conventional displayed transaction/block hash byte order |
| `Hex` | string | Non-empty, even-length lowercase hex without `0x` |
| `Base64` | string | Non-empty canonical padded RFC 4648 standard base64 |
| `Address` | string | Existing Purity-compatible address; no PWC-specific address prefix |
| `Network` | string | Network name matching `[a-z0-9-]{1,32}`; see below |
| `Cursor` | string | Opaque pagination/recovery marker; not an access credential |
| `Text` | string | Display text of 1 to 256 Unicode characters, without control characters (U+0000–U+001F, U+007F–U+009F) |

The monetary bound `2100000000000000` is the existing maximum money supply in satoshis (21,000,000 BTC). A value above the bound is a syntax error of the type.

`Network` values are compared by exact string equality. The names `mainnet`, `signet` and `regtest` are reserved for the corresponding existing Purity networks; a Provider that supports another existing network uses that network's established lowercase name. PWC registers no chain identity of its own.

An address is matched to a network only through its encoding. Where two networks share an encoding, such as the Base58 addresses of `signet` and `regtest`, an address of one is accepted for the other. Neither a PSBT nor a raw transaction carries a network identifier.

Reusable **composite types** are:

```typescript
type OutPoint = { txid: Hash256; vout: UInt32 };
type ScriptType = "p2pkh"|"p2sh"|"p2sh-p2wpkh"|"p2sh-p2wsh"|"p2wpkh"|"p2wsh"|"p2tr"|"other";
type MessageFormat = "bip322-simple"|"bip322-full"|"legacy";
```

`OutPoint` identifies one transaction output. `ScriptType` names an output script form; a Provider lists only forms valid on its networks. `UInt32`, `PositiveInteger` and `NonNegativeInteger` values are JSON integers, whereas amounts and fee rates are **always decimal strings**. JSON Schema uses `$ref` to refer to these types; it is not permissible for a schema generator to weaken their canonical syntax.

The companion JSON Schema (JSON Schema 2020-12) is distributed with this specification under the same version number. If the schema and this text disagree, this text prevails and the disagreement is a schema defect.

### 3.3 Identifiers and lifecycle

`wallet_id`, `account_id`, `client_id`, `session_id`, `grant_id`, `operation_id` and `event_id` are distinct domains. A Provider MUST NOT infer permissions from knowing an identifier. Identifiers SHOULD be random, opaque and free of derivation paths, xpubs, names or transaction hashes. They MAY change after reinstallation or restoration and MUST NOT be assumed portable across devices. Account mapping across devices is not a Core guarantee. `client_id` is assigned by the transport profile that authenticates the Client (§7).

### 3.4 Monetary values, fee rates and rounding

PWC amounts use **satoshis**; the existing `BTC` display unit and `1 BTC = 100,000,000 sat` relationship remain unchanged. `FeeRate` uses **sat/kvB**, defined as satoshis per **1,000 virtual bytes**. The wire value is a strictly positive integer decimal string and contains no fractional point, suffix or unit label. For example, `"1250"` means `1.25 sat/vB` (numerically the same value as `1250 msat/vB`, but PWC does not expose an msat unit).

*Informative:* some node RPC interfaces express fee rates in BTC/kvB. A PWC `FeeRate` is always sat/kvB; `"1250"` corresponds to `0.0000125` BTC/kvB.

Virtual size `vsize` is defined by BIP-141 as the transaction weight divided by four, rounded up. For a target virtual size `vsize` and rate `r = FeeRate`, the nominal minimum fee in satoshis is:

```text
fee_sats = ceil(r * vsize / 1000)
         = (r * vsize + 999) // 1000     // only if intermediate arithmetic cannot overflow
```

Implementations MUST use exact, overflow-safe integer arithmetic, or an equivalent overflow-safe quotient/remainder formula. They MUST NOT use binary floating point to calculate, round or silently truncate requested fee rates. If a fee-rate target is applied to an *estimated* signed size, the Provider MUST still enforce its declared wallet fee policy and MUST disclose the resulting fee in the `psbt.create` result (§4.7). Transaction byte size and transaction *virtual* size are not interchangeable. Zero or negative fee-rate wire values MUST be rejected; wallet policy MAY independently reject positive rates below its minimum.

`SignedSats` is used only in explicitly signed delta fields. No PWC balance, fee or amount field implies a fiat price or exchange rate.

### 3.5 Field disclosure

- Omitted data MUST be distinguished from a numeric zero.
- Providers MUST omit or redact information outside the current grant's account scope.
- Descriptors, extended public keys, derivation paths, public-key fingerprints and address history are potentially privacy-sensitive and MUST NOT be added to general balance, address, UTXO or transaction responses. The only place key-origin data may appear is inside a PSBT, under the limits of §4.7.
- Labels, memo text and Client-supplied descriptions MUST be treated as untrusted display text.
- A response MUST declare its network and/or account context wherever a result could otherwise be confused between wallets.

## 4. Canonical resource models

The types below are transport-independent logical interfaces. Each named semantic type uses the constraints in §3.2; the companion JSON Schema is the machine-readable definition. `?` denotes an optional field that is omitted when absent; it is never `null`. Inline literals enumerate exact allowed values.

```typescript
type AccountId = Id;              // documentation alias only; wire type remains Id
type SyncStatus = { state: "synced"|"syncing"|"offline"|"unknown";
                    tip_height?: BlockHeight };
type FeePolicy = { type: "rate"; sat_per_kvb: FeeRate }
               | { type: "wallet_estimate"; target_blocks: PositiveInteger };
```

### 4.1 `Capabilities`

```typescript
type Capabilities = {
  pwc_version: string;                 // semantic version of this specification, e.g. "0.2.1"
  provider: { name: Text; version: Text;
              platform: "desktop"|"mobile"|"extension"|"other" };
  profiles: string[];                  // conformance profiles of §5, e.g. "pwc-core-read-v1"
  supported_networks: Network[];
  methods: string[];                   // exact registered method names
  features: string[];
  scopes: string[];                    // every scope of §6 this Provider can grant
  script_types: ScriptType[];          // script forms the Provider can handle
  message_formats: MessageFormat[];    // empty if message.sign is not implemented
  limits: { max_page_size: PositiveInteger;
            max_psbt_bytes: PositiveInteger;          // decoded size of one PSBT
            max_combine_psbts: PositiveInteger;       // at least 2
            max_raw_transaction_bytes: PositiveInteger;
            max_message_bytes: PositiveInteger;       // UTF-8 length of a message to sign
            max_concurrent_operations: PositiveInteger;  // nonterminal Operations per Client
            operation_retention_seconds: PositiveInteger;
            high_fee_rate: FeeRate };  // threshold of the high-fee warning, §8.3
};
```

Core-defined feature identifiers are `coin_control`, `multisig`, `hardware_signing`, `watch_only` and `event_push`. `coin_control` means that `psbt.create` honors `selected_utxos`. `event_push` MUST NOT be advertised unless the Provider implements a transport profile that delivers the events of §8.6. Features indicate implementation capabilities, not permission. A Provider MAY advertise additional identifiers named as in §11. Supported methods and versions MUST be discovered, not inferred from the platform name or wallet kind.

`Capabilities` describes the Provider software, not the User's wallets. It is identical before and after any grant and MUST NOT disclose wallet inventory, account properties or any other User-specific information. `supported_networks`, `script_types` and `message_formats` therefore list what the software can handle; what a particular account supports is reported by `AccountSummary`.

### 4.2 `WalletSummary` and `AccountSummary`

```typescript
type WalletSummary = {
  wallet_id: Id;
  display_name: Text;
  network: Network;
  wallet_kind: "software"|"hardware"|"multisig"|"watch_only"|"other";
  state: "ready"|"locked"|"syncing"|"unavailable";
  account_ids: Id[];
};
type AccountSummary = {
  account_id: Id;
  wallet_id: Id;
  display_name: Text;
  network: Network;
  can_sign: boolean;
  script_types: ScriptType[];
  methods: string[];                   // registered account-scoped methods usable with this account
};
```

`wallet_kind` is display-oriented: wallet properties such as multisignature, hardware signing and watch-only access can overlap. Callers MUST rely on the account's `can_sign` and `methods`, not infer signing rights from this enum. `methods` is the subset of `Capabilities.methods` that takes an `account_id` and is supported for this account; a method absent from it returns `UNSUPPORTED_OPERATION` for this account. A Provider may contain zero currently accessible wallets. An accessible wallet without a native account model exposes one default account. `can_sign:true` means the account has an eligible signing path, not that an external signer is presently available; an account with `can_sign:false` MUST NOT list `psbt.sign` or `message.sign` in `methods`. `account_ids` lists only accounts disclosed by the current grant and MUST be empty while `state` is `locked` or `unavailable` (§8).

`state` means: `ready`, the wallet's data can be read and used; `syncing`, as `ready`, but balances and histories may be stale and are marked as described in §4.3; `locked`, the User must unlock the wallet before its account data can be read; `unavailable`, the wallet's data cannot currently be loaded, for example because its storage or signing device is missing. §8 defines how each state affects the methods.

### 4.3 `Balance`: chain snapshot and pending projection

```typescript
type Balance = {
  account_id: Id;
  network: Network;
  confirmed_sats: Sats;
  pending_self_sats: Sats;
  pending_external_sats: Sats;
  pending_outgoing_sats: Sats;
  unconfirmed_delta_sats: SignedSats;
  spendable_sats: Sats;
  locked_sats: Sats;
  immature_sats: Sats;
  updated_at: Timestamp;
  sync: SyncStatus;
};
```

**All fields are REQUIRED**; a component with no value is `"0"`. A Provider MUST evaluate them against one consistent snapshot consisting of (a) its selected verified chain tip and (b) its wallet-observed active, nonconflicting pending-transaction projection. A *pending projection* is the set of unconfirmed transactions the wallet currently treats as effective for its holdings after resolving known conflicts, replacements and descendants. It is a wallet observation, not a consensus claim about the global mempool.

Let `C` be the account-owned outpoints unspent **on the selected confirmed chain alone**, and `P` be account-owned unconfirmed outputs left unspent after applying the active pending projection. Let `D` be the members of `C` consumed by that projection. A pending transaction is *self-funded* if every one of its inputs spends an account-owned outpoint and every unconfirmed ancestor of it in the projection is also self-funded. Let `P_self` be the members of `P` created by self-funded transactions and `P_ext` the remaining members of `P`. Then:

- `confirmed_sats` = total value of `C`, **before** pending transactions are applied. Immature confirmed outputs, if any, remain included here.
- `pending_self_sats` = total value of `P_self`: unconfirmed outputs, typically **change**, that cannot be invalidated without a signature from this account's own keys.
- `pending_external_sats` = total value of `P_ext`: unconfirmed outputs that depend on at least one input the account does not control and can therefore be double-spent by another party. Neither pending field is gross received-payment volume; both count only outputs still unspent in the projection.
- `pending_outgoing_sats` = total value of `D`, i.e. confirmed outpoints consumed by the pending projection, **including their full input value**, not just payments to third parties. Pending-to-pending spends are not double-counted.
- `unconfirmed_delta_sats` = `pending_self_sats + pending_external_sats - pending_outgoing_sats`, serialized as `SignedSats`.
- The account's *effective value* is `confirmed_sats + unconfirmed_delta_sats`, the total of its effective unspent outputs `(C − D) ∪ P`. It MUST be nonnegative in a consistent snapshot. It is **not** a guarantee of spendability.
- `spendable_sats` = value of effective unspent outputs that the Provider's current policy lets `psbt.create` select. It MAY include zero-confirmation outputs the policy accepts. It does not depend on `can_sign`: a watch-only account reports what a PSBT for an external signer could spend.
- `locked_sats` = value of effective unspent outputs that are mature but blocked by a freeze, a lock or a PSBT reservation (§8.3).
- `immature_sats` = value of effective unspent coinbase outputs that are not yet mature.

Each effective unspent output is counted in **at most one** of `spendable_sats`, `locked_sats` and `immature_sats`; an output that is both immature and locked counts as immature. Outputs that are unavailable for another reason, such as unconfirmed outputs the policy will not spend, are counted in none of the three. Consequently `spendable_sats + locked_sats + immature_sats` MUST NOT exceed the effective value.

**Example:** At the confirmed tip, the account owns `1,000,000` sat. One pending transaction funded only by the account consumes a confirmed `400,000`-sat input, sends `300,000` sat externally, returns `99,000` sat as unconfirmed change and pays `1,000` sat fee. The snapshot is `confirmed_sats="1000000"`, `pending_self_sats="99000"`, `pending_external_sats="0"`, `pending_outgoing_sats="400000"`, `unconfirmed_delta_sats="-301000"` and effective value `699,000` sat. If a third party then sends the account an unconfirmed `50,000`-sat payment, `pending_external_sats` becomes `"50000"`, `unconfirmed_delta_sats` becomes `"-251000"` and the effective value `749,000` sat.

Conflicted, replaced or dropped transactions MUST NOT simultaneously contribute to the active pending projection. Once a replacement is accepted into the wallet's current projection, the replaced branch and any superseded descendants MUST be excluded; a reorganization triggers recomputation from the new chain view.

Staleness and incoherence are handled differently:

- If the Provider holds a coherent snapshot that may be out of date (it is still syncing, or its backend is offline), it MUST return that snapshot with `sync.state` other than `synced` and with `updated_at` set to the time the snapshot was last known current.
- If the Provider has never completed an initial synchronization of the account, it MUST return `SYNC_INCOMPLETE`.
- If an unresolved conflict, a missing transaction dependency or an inconsistent backend response prevents any coherent projection, it MUST return `STATE_UNAVAILABLE` instead of fabricating a zero, adding both conflicting branches, or silently selecting one.

This snapshot is a wallet-observed value, not a guarantee that every external pending transaction has been seen.

### 4.4 `AddressRecord`

```typescript
type AddressRecord = {
  account_id: Id;
  network: Network;
  address: Address;
  role: "receive"|"change"|"unknown";
  state: "unused"|"used"|"unknown";
  label?: Text;
};
```

An address does not prove ownership or receipt. Providers MUST use their existing native address/script rules. Derivation indexes and paths are not part of this record (§3.5).

### 4.5 `Utxo` and `OutPoint`

```typescript
type Utxo = OutPoint & {
  account_id: Id;
  network: Network;
  value_sats: Sats;
  confirmations: NonNegativeInteger;
  block_height?: BlockHeight;
  address?: Address;
  script_pub_key?: Hex;
  spendable: boolean;
  restrictions: ("frozen"|"locked"|"immature"|"unconfirmed"|"unknown")[];
  is_change?: boolean;
};
```

`OutPoint` is the reusable pair `(txid, vout)` used also by explicit PSBT input selection. `confirmations` is zero for unconfirmed outputs; `block_height` is present exactly when `confirmations` is at least 1. `spendable` has the meaning of `spendable_sats` in §4.3 and does not depend on `can_sign`. `spendable:true` requires an empty `restrictions` list; `spendable:false` requires a nonempty one. `locked` covers wallet locks and PSBT reservations. `unconfirmed` is listed only when local policy refuses to spend the output because it is unconfirmed; an unconfirmed output the policy accepts has `confirmations:0` and an empty list. UTXO responses MUST be constructed from an explicit field allowlist, never by serializing an internal wallet object containing private keys, derivation secrets or implementation-only fields.

### 4.6 `TransactionRecord`

```typescript
type TransactionRecord = {
  account_id: Id;
  network: Network;
  txid: Hash256;
  status: "unconfirmed"|"confirmed"|"conflicted"|"replaced"|"dropped"|"unknown";
  confirmations: NonNegativeInteger;
  block_height?: BlockHeight;
  first_seen_at?: Timestamp;
  confirmed_at?: Timestamp;
  net_change_sats: SignedSats;
  fee_sats?: Sats;
  replaced_by_txid?: Hash256;
};
```

`status` is `confirmed` exactly when `confirmations` is at least 1, and `block_height` is present exactly in that case. `confirmed_at` is the timestamp of the confirming block header and is informational. `net_change_sats` is account-relative and MUST be computed from the transaction's actual owned inputs/outputs, not a Client-provided amount. A transaction can move between states after a chain reorganization. `dropped`, `conflicted` and `replaced` describe wallet observations; none is a separate consensus state. `fee_sats` is present only when the Provider knows every input value and can therefore determine the fee exactly.

### 4.7 `PsbtObject`, partial signing, combination and finalization

```typescript
type PsbtObject = {
  psbt: Base64;
  psbt_version: PsbtVersion;
};
type AccountPsbtObject = PsbtObject & {
  account_id: Id;
  network: Network;
};
type CreatePsbtResult = AccountPsbtObject & {
  fee_sats: Sats;
  vsize: PositiveInteger;              // estimated virtual size once fully signed
  change_output_indexes: UInt32[];     // outputs paying back to the account; may be empty
  reservation_id?: Id;                 // present exactly when the inputs are reserved, §8.3
  reserved_until?: Timestamp;          // present exactly when the inputs are reserved
};
type SignPsbtResult = AccountPsbtObject & {
  signed_input_indexes: UInt32[];
  signing_status: "partial"|"complete"|"unknown";
};
type CombinePsbtResult = PsbtObject;
type FinalizePsbtResult = PsbtObject & {   // psbt is the updated, or unchanged, PSBT
  complete: boolean;
  raw_transaction?: Hex;               // present exactly when complete is true
  txid?: Hash256;                      // present exactly when complete is true
};
```

A `PsbtObject` contains **data**, not a wallet authorization. Only account-scoped creation/signing results have `account_id` and `network`. Account-independent `psbt.combine` and `psbt.finalize` operate on supplied PSBT bytes; they do not assert that the PSBT belongs to any account/network, access spending keys, or select a chain backend. Before signing or broadcasting, a method that accesses an account MUST independently enforce that account's existing network context.

**PSBT version.** PWC supports only PSBT version 0 (BIP-174): a PSBT without `PSBT_GLOBAL_VERSION`, or with that field set to 0. Every method that accepts a PSBT MUST reject any other version with `UNSUPPORTED_PSBT_VERSION` and MUST NOT convert it. Every PSBT a Provider returns is version 0 and carries `psbt_version: 0`; the field exists so that later versions of this specification can add PSBT versions.

**PSBT-embedded disclosure.** A PSBT returned by `psbt.create` or `psbt.sign` necessarily reveals the transaction's inputs and may carry key-origin data that cosigners and hardware signers need. The Provider MAY include, for the inputs and outputs of **that transaction only**: previous-output data, redeem/witness scripts, and BIP-32 derivation or Taproot key-origin fields. It MAY include `PSBT_GLOBAL_XPUB` only for a multisignature account whose cosigners require it. It MUST NOT add data about any other UTXO, address, account or wallet. The consent prompt for a grant containing `psbt:create` MUST state that created PSBTs disclose the selected coins and their key-origin data. `psbt.sign` MAY add to a Client-supplied PSBT only signatures and, for the inputs it signs and the outputs paying to the account, the fields permitted above.

`psbt.sign` MUST support **partial signing** and return the updated PSBT. `signed_input_indexes` lists, in ascending order, the unique input indexes to which this invocation added at least one valid signature; it does not count signatures per key. It is empty only when every input targeted by the request already carried all signatures this account can contribute; that outcome is a successful result, not an error. `signing_status="complete"` asserts that all inputs' applicable signature conditions are known to be satisfied; `"partial"` asserts at least one is known to remain unsatisfied; `"unknown"` means completeness cannot safely be determined. **None of these statuses implies PSBT finalization or network broadcast.** Signing MUST NOT silently finalize, extract or broadcast. Existing valid partial signatures and safely supported proprietary/unknown PSBT fields MUST be retained; otherwise the request MUST fail rather than silently discard data.

`psbt.combine` merges PSBTs for **the same transaction**, without signing, finalizing, extracting or broadcasting. "The same transaction" means that the global unsigned transactions are byte-for-byte identical. PSBTs that are not the same transaction MUST cause `PSBT_CONFLICT`. Within the same transaction, the result is the union of all key-value pairs. Where two PSBTs hold different values for the same key:

- **Signature fields** (`PSBT_IN_PARTIAL_SIG`, `PSBT_IN_TAP_KEY_SIG`, `PSBT_IN_TAP_SCRIPT_SIG`): if every differing value verifies against the transaction and the PSBT's previous-output data, the value from the earliest submitted PSBT is kept; otherwise the request fails with `PSBT_CONFLICT`. Two valid signatures by the same key can legitimately differ, for example through different nonce generation.
- **Any other key:** the request MUST fail with `PSBT_CONFLICT`.

Implementations MUST NOT otherwise silently prefer one value. PSBT combination is distinct from *joining* different transactions.

`psbt.finalize` attempts to populate final input scripts/witnesses using existing signatures only. With `complete:false` the result carries the updated PSBT, or the unchanged PSBT if no input could be finalized. `complete:true` means that every input was finalized, that each final script and witness verified against the previous-output data **contained in the PSBT**, and that the transaction was extracted; the result then carries `raw_transaction`, its `txid` and the finalized PSBT. It is not a statement that the inputs exist or are unspent on any chain. If an input lacks the previous-output data needed for verification, the method MUST fail with `PSBT_VALIDATION_FAILED` rather than report `complete:true`. Finalization MUST NOT create signatures or broadcast. Combination and finalization MUST be bounded, deterministic for identical inputs and independently safe to expose to an authenticated, rate-limited Client without any wallet-account grant.

### 4.8 `Grant`, `SessionInfo`, `Operation` and `PwcEvent`

```typescript
type GrantConstraints = {
  max_amount_sats_per_request?: PositiveSats;
  valid_until?: Timestamp;
};
type Grant = {
  grant_id: Id;
  client_id: Id;
  network: Network;
  scopes: string[];
  wallet_ids: Id[];
  account_ids: Id[];
  created_at: Timestamp;
  expires_at?: Timestamp;
  status: "active"|"revoked"|"expired";
  constraints?: GrantConstraints;
};
type SessionInfo = {
  session_id: Id;
  client_id: Id;
  grant_ids: Id[];
  created_at: Timestamp;
  expires_at?: Timestamp;
};
type Operation = {
  operation_id: Id;
  method: string;
  state: "queued"|"awaiting_approval"|"awaiting_unlock"|"awaiting_signer"|
         "processing"|"completed"|"rejected"|"expired"|"canceled"|"failed";
  created_at: Timestamp;
  updated_at: Timestamp;
  expires_at?: Timestamp;             // REQUIRED while queued or awaiting, §9.1
  account_id?: Id;
  result?: object;     // present exactly when state is "completed", the caller remains authorized and the wallet is neither locked nor unavailable
  error?: PwcError;    // present exactly when state is "rejected", "expired", "canceled" or "failed"
};
type PwcEvent = {
  event_id: Id;
  event_type: string;  // one of the names in §8.6, or an extension name (§11)
  occurred_at: Timestamp;
  account_id?: Id;
  operation_id?: Id;
  payload: object;     // per event type, §8.6
};
```

`client_id` identifies an authenticated principal established by a transport profile, not an arbitrary caller-chosen app name. A grant has these semantics:

- **Network.** A grant is bound to exactly one `network`. Every wallet and account it covers is on that network. A request for an account on another network fails as an unknown account (§8).
- **Resources.** A wallet listed in `wallet_ids` is covered together with **all of its accounts**, including accounts created after the grant. `account_ids` lists accounts granted individually from wallets that are not in `wallet_ids`. The covered accounts are the union of both. `Grant.account_ids` is the grant's own record: it never lists the accounts of a wallet in `wallet_ids`, and `session.getPermissions` returns it whatever the state of the wallet.
- **Expiry.** The grant expires at the earlier of `expires_at` and `constraints.valid_until`; if only one is present it applies, and if neither is present the grant lasts until revoked. From that instant `status` is `expired`.
- **Amount limit.** `max_amount_sats_per_request` limits `psbt.create` and `psbt.sign`. The measured amount is the account's net outflow in the transaction: the total value of its account-owned inputs minus the total value of its outputs paying back to the same account. For a transaction funded only by the account this equals the external payments **plus the miner fee**. A negative net outflow counts as zero. A request whose amount exceeds the limit MUST fail with `GRANT_CONSTRAINT_VIOLATED` before any approval prompt is shown. If the amount cannot be determined, the request fails with `PSBT_VALIDATION_FAILED`.

Grant constraints restrict requests; they never confer unattended signing authority. A revoked/expired grant MUST immediately prevent new sensitive accesses. A pre-grant permission request Operation remains readable by its original authenticated requester without revealing selected accounts prior to consent.

## 5. Capability and mandatory method profiles

A Provider lists every profile it conforms to in `Capabilities.profiles`. The suffix of a profile name, such as `-v1`, is the revision of that profile and is independent of `pwc_version`; it changes only when the profile's required methods or semantics change incompatibly.

### 5.1 `pwc-core-read-v1` (mandatory)

A conforming Provider MUST implement `pwc.getCapabilities`, `session.getInfo`, `session.requestPermissions`, `session.getPermissions`, `session.revokePermissions`, `wallet.list`, `account.list`, `account.getBalance`, `address.getReceiveAddress`, `transaction.list`, `operation.get` and `operation.cancel`. The Provider MAY have zero currently accessible wallets and MUST then return an empty inventory honestly. An account with a fixed address or inaccessible address derivation MAY return `UNSUPPORTED_OPERATION` for `address.getReceiveAddress`, while the method remains implemented.

### 5.2 `pwc-psbt-signer-v1` (optional)

A Provider advertising this profile MUST implement `psbt.sign`, support the partial-signing result semantics of §4.7 and §8.3, and list the script types it can sign in `Capabilities.script_types`. `psbt.create`, `psbt.combine`, `psbt.finalize` and `transaction.broadcast` are individually advertised optional methods, not implied by signing support. Signing ability is a property of each account (`can_sign`, §4.2), not of the profile.

### 5.3 Individually advertised optional methods

`utxo.list`, `address.list`, `address.create`, `transaction.get`, `psbt.create`, `psbt.releaseReservation`, `psbt.combine`, `psbt.finalize`, `transaction.broadcast`, `message.sign` and `message.verify` are optional. A Provider that reserves PSBT inputs (§8.3) MUST implement `psbt.releaseReservation`. Providers MUST advertise each supported method exactly. Features are not permissions. Event vocabulary is specified by Core, while event transport and subscriptions are deferred.

## 6. Permission scopes

| Scope | Grants access to | Approval behavior |
|---|---|---|
| `wallets:read` | Wallet/account inventory | Explicit initial disclosure consent |
| `balances:read` | `account.getBalance` | Grant required |
| `addresses:read` | Existing/current addresses | Grant required |
| `addresses:create` | New receive address | Explicit grant; no payment implied |
| `transactions:read` | Account transaction history | Grant required |
| `utxos:read` | Outpoints and coin-selection metadata | Privacy-sensitive grant |
| `psbt:create` | Construct PSBT using an account's resources; release its reservations | Grant plus local wallet policy; discloses the selected coins (§4.7) |
| `psbt:sign` | Request signatures for selected inputs | **Individual approval of every transaction** |
| `transactions:broadcast` | Submit raw transaction to wallet-selected backend | Grant plus applicable local policy |
| `messages:sign` | Request supported message signature | **Individual approval of every message** |
| `events:read` | Deliver authorized state-change events | Underlying read scopes still enforced |

`psbt.combine`, `psbt.finalize` and `message.verify` are **pure data operations**: they require an authenticated session and are subject to resource and rate limits, but need **no scope and no wallet/account grant**, and confer no signing authority. This does not authorize broadcasting their output. The `session.*`, `operation.*` and `pwc.getCapabilities` methods likewise need an authenticated session but no scope.

A permission is the intersection of authenticated Client, active session, active grant, wallet/account scope where relevant, supported method, current wallet state and applicable user policy. Grant IDs and session connectivity are never spending authority. Users MUST be able to review and revoke grants. Unattended signing is not part of this version.

## 7. Abstract session lifecycle (transport-independent)

1. **Discovery:** A transport profile identifies a Provider and establishes a secure authenticated channel. Discovery MUST NOT disclose wallet inventory, addresses or balances to unauthenticated websites/processes.
2. **Identity:** The transport establishes `client_id`, verifies possession of Client credentials and binds the principal to the channel. Names/icons are untrusted display metadata.
3. **Request permission:** `session.requestPermissions` requests scopes and account choices and always returns an Operation. The Provider displays identity and scope information and obtains User consent.
4. **Grant:** On approval, the Provider creates a revocable `Grant` bound to `client_id`, network, account scope and policy. A session never creates an unconditional signing right.
5. **Use:** Every business method checks permission *when executed*, not merely when queued. High-risk actions undergo separate transaction/message review.
6. **Revocation/expiry:** New actions fail immediately. Pending Operations that depend on the grant MUST be re-evaluated before execution and, if no longer authorized, end in state `failed` with `PERMISSION_REVOKED`; already broadcast transactions cannot be reversed.
7. **Recovery:** After reconnection, the Client re-authenticates and may fetch its operation state within the retention policy. Request retries MUST follow the idempotency rules in §9.3.

Transport-specific pairing tokens, key formats, QR codes, handshakes, cryptographic algorithms, browser origins and URL callbacks are reserved for future profiles. A transport profile MUST NOT use mere knowledge of an account ID as authentication.

## 8. Business API registry

Each method below accepts a named `params` object.

**Order of checks.** A Provider MUST evaluate a request in this order and return the error of the first check that fails. Checks 6 and 7 apply only to methods that require a scope (§6).

1. Authenticated session, else `UNAUTHENTICATED`.
2. Request quota: a Provider MAY refuse a Client that exceeds its request quota with `RATE_LIMITED`.
3. Method implemented, else `UNSUPPORTED_METHOD`.
4. Parameter syntax, else `INVALID_ARGUMENT`; size limits of §4.1, else `PAYLOAD_TOO_LARGE`.
5. Idempotency, for methods with an `idempotency_key` (§9.3): if the key already has a record for this Client and method, an exact retry is answered with the original response and no further check is made; for an Operation the response is its current state under the visibility rules of §8.5. A retry with different parameters fails with `IDEMPOTENCY_CONFLICT`.
6. Required scope held by an active grant of this Client. Otherwise the request fails with `PERMISSION_REVOKED` if a revoked or expired grant of this Client that the Provider still retains held the scope, and with `PERMISSION_REQUIRED` if none did.
7. Referenced wallet or account exists **and** is covered by an active grant holding that scope, else `WALLET_NOT_FOUND` or `ACCOUNT_NOT_FOUND`. A resource outside the grant and a nonexistent resource MUST produce the same error with the same content, so that the response does not reveal whether the resource exists.
8. Wallet state, under the rules for locked and unavailable wallets below.
9. Concurrency, for methods that create an Operation: `limits.max_concurrent_operations`, else `RATE_LIMITED`.
10. Method-specific checks, including grant constraints (§4.8).

**Locked and unavailable wallets.** While a wallet's `state` is `locked`, its account data MUST NOT be readable. `wallet.list` still returns the wallet's `WalletSummary` with an empty `account_ids`. `account.list`, `account.getBalance`, every `address.*` method, `utxo.list`, `transaction.list` and `transaction.get` for that wallet MUST fail with `WALLET_LOCKED`. The Operation-returning methods `psbt.create`, `psbt.sign`, `message.sign` and `transaction.broadcast` MUST either return an Operation in state `awaiting_unlock`, when the Provider can ask the User to unlock through trusted UI, or fail with `WALLET_LOCKED`; they MUST NOT disclose account data before the unlock. `pwc.getCapabilities`, `session.*`, `operation.*`, `psbt.combine`, `psbt.finalize`, `psbt.releaseReservation` and `message.verify` do not read account data and are otherwise unaffected; however, `operation.get` returns an Operation concerning such a wallet without `result`, and an exact retry answered at check 5 obeys the same rule, failing with `WALLET_LOCKED` if its stored synchronous response contains that wallet's account data. While `state` is `unavailable`, the same rules apply with `STATE_UNAVAILABLE` in place of `WALLET_LOCKED`, `wallet.list` returns an empty `account_ids`, and the Operation-returning methods fail directly with `STATE_UNAVAILABLE`. A wallet in state `syncing` is used as one in state `ready`, subject to the staleness rules of §4.3.

**Pagination.** The default page size is the smaller of 50 and the advertised `max_page_size`. A requested `page_size` above `max_page_size` is reduced to `max_page_size`. Clients MUST use the `next_cursor` provided by a Provider rather than synthesize page tokens. A cursor is valid only for the method, account and Client it was issued to; any other use MUST fail with `INVALID_ARGUMENT`. The absence of `next_cursor` marks the last page.

### 8.1 Discovery, grants and accounts

```typescript
type RequestPermissionsParams = {
  scopes: string[];                    // at least one
  network?: Network;                   // omitted: the User chooses in the wallet
  wallet_ids?: Id[];
  account_ids?: Id[];
  expires_at?: Timestamp;              // requested expiry
  constraints?: GrantConstraints;      // requested constraints
  idempotency_key: Id;
};
```

| Method | Params | Result | Scope / notes |
|---|---|---|---|
| `pwc.getCapabilities` | `{}` | `Capabilities` | Same result before and after any grant; no User data |
| `session.getInfo` | `{}` | `SessionInfo` | Authenticated session |
| `session.requestPermissions` | `RequestPermissionsParams` | `Operation`, completed with `{ grants: Grant[] }` | Resource lists may be empty or omitted; the User chooses in the wallet |
| `session.getPermissions` | `{}` | `{ grants: Grant[] }` | Only grants belonging to the authenticated Client |
| `session.revokePermissions` | `{ grant_id: Id, idempotency_key: Id }` | `{ revoked: boolean }` | Client can revoke its own grant; the User can revoke from wallet UI |
| `wallet.list` | `{}` | `{ wallets: WalletSummary[] }` | `wallets:read`; only authorized wallets |
| `account.list` | `{ wallet_id: Id }` | `{ accounts: AccountSummary[] }` | `wallets:read`; only authorized accounts |

`session.requestPermissions` always returns an Operation. The User, not the Client, decides the final grant: the Provider MAY grant fewer scopes, fewer resources, an earlier expiry or tighter constraints than requested, and MUST NOT grant more scopes, a later expiry or looser constraints than requested. When the Operation completes, its `result` is `{ "grants": Grant[] }` with the grants created or confirmed by this request; a refused request ends in state `rejected`. The Provider MAY complete the Operation without a prompt only if an active grant of the same Client already covers every requested scope and resource with constraints at least as tight as requested.

`session.getPermissions` returns every active grant of the Client; it MAY also return revoked or expired grants for as long as the Provider retains them.

`session.revokePermissions` returns `revoked:true` if the call changed an active grant to `revoked`, and `revoked:false` if the grant was already revoked or expired. A `grant_id` that does not exist or belongs to another Client MUST fail with `GRANT_NOT_FOUND`.

The `wallet_ids`/`account_ids` in a permission request are usable only if already known from a previous authorized session; a first-time Client omits them and the User picks in the wallet. Identifiers the Provider does not recognize MUST be ignored silently rather than reported, so that the request cannot probe for wallets.

### 8.2 Balances, addresses, UTXOs and history

| Method | Params | Result | Scope / notes |
|---|---|---|---|
| `account.getBalance` | `{ account_id: Id }` | `Balance` | `balances:read` |
| `address.getReceiveAddress` | `{ account_id: Id }` | `AddressRecord` | `addresses:read`; returns current receive address; no derivation advancement promised |
| `address.create` | `{ account_id: Id, label?: Text, idempotency_key: Id }` | `AddressRecord` | `addresses:create`; new address if wallet supports derivation |
| `address.list` | `{ account_id: Id, cursor?: Cursor, page_size?: PositiveInteger }` | `{ addresses: AddressRecord[], next_cursor?: Cursor }` | `addresses:read` |
| `utxo.list` | `{ account_id: Id, cursor?: Cursor, page_size?: PositiveInteger }` | `{ utxos: Utxo[], next_cursor?: Cursor }` | `utxos:read` |
| `transaction.list` | `{ account_id: Id, cursor?: Cursor, page_size?: PositiveInteger }` | `{ transactions: TransactionRecord[], next_cursor?: Cursor }` | `transactions:read`; newest-first by wallet observation ordering |
| `transaction.get` | `{ account_id: Id, txid: Hash256 }` | `{ transaction: TransactionRecord }` | `transactions:read` |

`address.getReceiveAddress` is permitted to return the same current address repeatedly. Applications requiring a fresh address MUST use `address.create` with an idempotency key and `addresses:create` scope. A Provider MUST NOT return another account's xpubs or addresses in an account-scoped response. `transaction.get` for a txid the account has no record of fails with `TRANSACTION_NOT_FOUND`. Pagination consistency is best-effort under reorgs: stale cursors may return `CURSOR_EXPIRED`, and the Client restarts listing.

### 8.3 PSBT and transaction lifecycle

| Method | Params | Direct result | Scope / notes |
|---|---|---|---|
| `psbt.create` | `CreatePsbtParams` | `Operation`, completed with `CreatePsbtResult` | `psbt:create`; optional |
| `psbt.releaseReservation` | `ReleaseReservationParams` | `{ released: boolean }` | `psbt:create`; **synchronous**; optional |
| `psbt.sign` | `SignPsbtParams` | `Operation`, completed with `SignPsbtResult` | `psbt:sign`; per-transaction approval |
| `psbt.combine` | `CombinePsbtParams` | `CombinePsbtResult` | Authenticated, account-independent, **synchronous** |
| `psbt.finalize` | `FinalizePsbtParams` | `FinalizePsbtResult` | Authenticated, account-independent, **synchronous** |
| `transaction.broadcast` | `BroadcastParams` | `Operation`, completed with `{ txid: Hash256 }` | `transactions:broadcast`; optional |

```typescript
type CreatePsbtParams = {
  account_id: Id;
  outputs: { address: Address; amount_sats: PositiveSats }[];   // at least one
  fee_policy: FeePolicy;
  selected_utxos?: OutPoint[];
  replaceable?: boolean;               // BIP-125 signaling; omitted: wallet default
  idempotency_key: Id;
};
type ReleaseReservationParams = {
  account_id: Id;
  reservation_id: Id;
  idempotency_key: Id;
};
type SignPsbtParams = {
  account_id: Id;
  psbt: Base64;
  input_indexes?: UInt32[];            // omitted: every input the account can sign
  idempotency_key: Id;
};
type CombinePsbtParams = { psbts: Base64[] };   // 2 to limits.max_combine_psbts entries
type FinalizePsbtParams = { psbt: Base64 };
type BroadcastParams = {
  account_id: Id;
  raw_transaction: Hex;
  idempotency_key: Id;
};
```

`psbt.create` MUST validate the requested `FeePolicy`, the applicable wallet coin-selection rules and the selected network/account. The `sat_per_kvb` field MUST contain a `FeeRate` (§3.4). Explicit `selected_utxos` requests MUST NOT be silently ignored; a Provider without the `coin_control` feature MUST fail them with `UNSUPPORTED_OPERATION`, and an unavailable outpoint fails with `INVALID_ARGUMENT`. An output address encoded for a network other than the account's fails with `NETWORK_MISMATCH`; a string that is not a valid address on any supported network fails with `INVALID_ARGUMENT`. Insufficient eligible value fails with `INSUFFICIENT_FUNDS`, a fee rate the wallet policy refuses with `POLICY_VIOLATION`, and a `wallet_estimate` the wallet cannot currently compute with `STATE_UNAVAILABLE`. This basic payment builder requires positive output amounts. It MUST NOT sign, finalize or broadcast. The result discloses the exact `fee_sats`, the estimated `vsize` and the positions of the change outputs.

**Reservation.** A Provider MAY reserve the inputs of a created PSBT so that concurrent requests do not select them. If it does, the result MUST carry `reservation_id` and `reserved_until`, and the inputs are reported as `locked` (§4.3, §4.5) until the reservation ends. `reserved_until` MUST NOT be later than 24 hours after creation, and a Provider MUST NOT hold a reservation without an expiry. A reservation ends at the first of these events:

- `reserved_until` passes;
- the Client releases it with `psbt.releaseReservation`;
- a transaction spending any reserved input enters the wallet's pending projection;
- the User spends a reserved output from the wallet's own interface;
- the grant under which the PSBT was created is revoked or expires.

A reservation restricts only selection by PWC requests. It MUST NOT prevent the User from spending reserved outputs in the wallet itself.

`psbt.releaseReservation` ends a reservation that the same Client created for the given account. It returns `released:true` if the call ended an active reservation, and `released:false` if the reservation had already ended. The Provider MUST remember an ended reservation for at least `limits.operation_retention_seconds`. A `reservation_id` that is unknown, no longer remembered, or belongs to another Client or account MUST fail with `RESERVATION_NOT_FOUND`, with the same content in every case. Releasing does not alter or invalidate the PSBT; it only makes its inputs selectable again.

`psbt.sign` MUST parse the PSBT under supported BIP rules, identify which inputs can be signed by the selected account, and validate input amounts/scripts, outputs, fees, change and sighash modes sufficiently for secure review. The User MUST approve the exact PSBT intent and **approved input subset**. The following rules apply, and the checks they describe are made in this order: PSBT parsing and version; `input_indexes`; signability; script types and sighash modes; review validation; grant constraints; high-fee warning.

- `input_indexes`, when given, MUST be nonempty and contain unique in-range indexes; otherwise the request fails with `INVALID_ARGUMENT`.
- If any index in `input_indexes` designates an input the account cannot sign, or if `input_indexes` is omitted and the account can sign no input, the request fails with `UNSUPPORTED_OPERATION` and nothing is signed. An input whose previous output is not an account-owned output on the account's network is an input the account cannot sign.
- The Provider MUST NOT sign any input outside the requested set or silently enlarge the selection. If input-specific signing cannot be enforced, it fails with `UNSUPPORTED_OPERATION`.
- A PSBT of a version other than 0 fails with `UNSUPPORTED_PSBT_VERSION`; an unsupported script type or sighash mode with `UNSUPPORTED_SCRIPT`; an unparsable, unreviewable or invalid PSBT with `PSBT_VALIDATION_FAILED`. The Provider MUST NOT blind-sign.
- **High-fee warning.** If the transaction's fee rate exceeds `limits.high_fee_rate`, or its fee cannot be determined, the approval prompt MUST carry an explicit high-fee warning that the User acknowledges separately, or the Provider MUST refuse with `POLICY_VIOLATION`. The same applies to any sighash mode other than the default for the input's script type.

A successful signing result MAY be partial, fully signed or have unknown overall completeness, as defined in §4.7. It MUST preserve the same transaction, MUST NOT finalize and MUST NOT broadcast. Unlocking or attaching a signer is not a substitute for transaction consent.

`psbt.combine` MUST apply the rules of §4.7: it verifies that every submitted PSBT is version 0 and that all are the same transaction, merges nonconflicting signatures and metadata, preserves safely supported unknown/proprietary fields and returns a version 0 PSBT. A PSBT of another version fails with `UNSUPPORTED_PSBT_VERSION`; different transactions and conflicting values for the same key fail with `PSBT_CONFLICT`. It MUST NOT join distinct transactions, sign, finalize, extract, broadcast or mutate wallet state. No `account_id` or `idempotency_key` is required for this pure operation.

`psbt.finalize` MUST NOT sign or broadcast. It attempts script/witness finalization using the PSBT's existing data and returns the result defined in §4.7. It requires neither `account_id` nor `idempotency_key`. For both pure PSBT methods, no chain identity is inferred from the PSBT bytes; subsequent account-scoped signing or broadcasting MUST independently check the wallet's existing network context.

`transaction.broadcast` MUST independently parse the complete raw transaction and apply wallet policy. A raw transaction carries no network identifier; one whose inputs do not exist on the account's network is refused by the backend. The Operation reaches `completed` with the `txid` only when the selected backend accepted the transaction; acceptance is **not** a confirmation. If the backend refuses the transaction, the Operation ends in state `failed` with `TRANSACTION_REJECTED`; if the backend cannot be reached, with `NETWORK_UNAVAILABLE`. Submitting a transaction the backend already holds or has confirmed completes successfully with its `txid`. The Provider MAY refuse, with `POLICY_VIOLATION`, to broadcast transactions not attributed to the selected account. Idempotent retries MUST NOT initiate multiple independently authorized broadcast operations.

### 8.4 Optional message signing

| Method | Params | Result | Scope |
|---|---|---|---|
| `message.sign` | `SignMessageParams` | `Operation`, completed with `SignMessageResult` | `messages:sign` and per-message approval |
| `message.verify` | `VerifyMessageParams` | `{ valid: boolean }` | Authenticated session; no scope (§6) |

```typescript
type SignMessageParams = {
  account_id: Id;
  address: Address;
  message: string;
  format: MessageFormat;
  idempotency_key: Id;
};
type SignMessageResult = { signature: Base64; format: MessageFormat; address: Address };
type VerifyMessageParams = {
  network: Network;
  address: Address;
  message: string;
  signature: Base64;
  format: MessageFormat;
};
```

The bytes signed or verified are exactly the UTF-8 encoding of `message` as received after JSON decoding; the Provider MUST NOT normalize, trim or otherwise alter them. A message longer than `limits.max_message_bytes` fails with `PAYLOAD_TOO_LARGE`. Arbitrary binary messages are not supported. A format absent from `Capabilities.message_formats` fails with `UNSUPPORTED_OPERATION`, as does an address whose script type the format cannot sign; an address the account does not own fails with `INVALID_ARGUMENT`, and an address encoded for a network other than the account's, or for `message.verify` other than the stated `network`, with `NETWORK_MISMATCH`. A `network` in `message.verify` that is not in `Capabilities.supported_networks` fails with `UNSUPPORTED_OPERATION`.

BIP-322 is preferred where implemented; the signature MUST be the base64 form defined for the selected BIP-322 variant. The `legacy` format is the existing compact message signature for `p2pkh` addresses (BIP-137 style) and uses the network's existing signed-message prefix unchanged. Message signing MUST present the exact message and address to the User; it is not an authorization to sign an arbitrary 32-byte transaction digest. Providers MAY omit message signing entirely. `message.verify` returns `valid:false` for a well-formed signature that does not verify, and an error only for malformed input. Clients MUST NOT infer that a valid message signature proves current UTXO ownership or asset balance.

### 8.5 Operation API

| Method | Params | Result | Scope / notes |
|---|---|---|---|
| `operation.get` | `{ operation_id: Id }` | `Operation` | Only originating authenticated Client; sensitive results require an active scope, except that pre-grant consent operations remain pollable by their requester |
| `operation.cancel` | `{ operation_id: Id, idempotency_key: Id }` | `Operation` | Cancellation is best-effort; irreversible actions return `OPERATION_NOT_CANCELABLE` |

Providers MUST retain every Operation while it is nonterminal and for at least `limits.operation_retention_seconds` after it reaches a terminal state. The advertised value SHOULD be at least 86,400 (24 hours). If an operation is no longer retained, was never created, or belongs to another Client, the method MUST return `OPERATION_NOT_FOUND` with the same content in all three cases. If the originating Client no longer holds the scope the operation required, `operation.get` returns the Operation without `result`. **Exception:** a `session.requestPermissions` Operation MUST be retrievable by its authenticated requester without requiring the grant it is seeking; it MUST NOT reveal the selected wallet/account before approval.

`operation.cancel` on a nonterminal Operation that has not passed an irreversible step moves it to `canceled`. On an Operation already in a terminal state it returns that Operation unchanged. On a nonterminal Operation past an irreversible step, such as a transaction already handed to the backend, it fails with `OPERATION_NOT_CANCELABLE`.

### 8.6 Event vocabulary (delivery deferred)

Core defines the following events. The event object is the `PwcEvent` type of §4.8. `account_id` and `operation_id` in the event envelope are present where the table says so.

| `event_type` | Envelope fields | `payload` |
|---|---|---|
| `wallet.locked` | none | `{ wallet_id: Id }` |
| `wallet.unlocked` | none | `{ wallet_id: Id }` |
| `wallet.syncChanged` | none | `{ wallet_id: Id, sync: SyncStatus }` |
| `account.balanceChanged` | `account_id` | `{}` |
| `account.transactionChanged` | `account_id` | `{ txid: Hash256, status: string }`, `status` as in `TransactionRecord` |
| `operation.stateChanged` | `operation_id` | `{ state: string }`, `state` as in `Operation` |
| `grant.revoked` | none | `{ grant_id: Id }` |
| `grant.expired` | none | `{ grant_id: Id }` |
| `session.expired` | none | `{ session_id: Id }` |

Event delivery, subscription API, replay cursors, guaranteed ordering and acknowledgement are **not** standardized here. Events are hints, not final sources of truth; on reconnection Clients SHOULD query authoritative state. An event MUST disclose no more information than an equivalent authorized read method, and is delivered only to a Client that could currently perform that read; in particular, `account.*` and `wallet.syncChanged` events are not delivered for a locked or unavailable wallet. A transport MAY support no events at all.

## 9. Asynchronous operations and approval state machine

### 9.1 States

- `queued`: accepted for processing, not yet acted upon.
- `awaiting_approval`: User must authorize this exact operation.
- `awaiting_unlock`: wallet is locked and the User must unlock it through trusted UI.
- `awaiting_signer`: hardware device or cosigner interaction is required.
- `processing`: wallet is executing the action.
- `completed`: the defined result is available.
- `rejected`: User explicitly refused.
- `expired`: operation's authorization window elapsed.
- `canceled`: canceled before an irreversible step.
- `failed`: internal, verification, network or policy failure after acceptance.

`expires_at` MUST be present while an Operation is `queued`, `awaiting_approval`, `awaiting_unlock` or `awaiting_signer`. If it passes before the Operation leaves these states, the Operation moves to `expired`.

`completed`, `rejected`, `expired`, `canceled` and `failed` are terminal. Providers MAY move between nonterminal states as needed. Every state change MUST update `updated_at`; once a terminal state is reached the operation MUST NOT return to a nonterminal state or change its `result` or `error`. A completed broadcast operation means submission was accepted by the selected backend, not mined inclusion. A completed `psbt.sign` means an updated PSBT was produced, not that all required parties signed.

Every terminal state other than `completed` carries an `error`:

| Terminal state | `error.code` |
|---|---|
| `rejected` | `USER_REJECTED` |
| `expired` | `OPERATION_EXPIRED` |
| `canceled` | `OPERATION_CANCELED` |
| `failed` | The code of §10 that describes the failure, e.g. `TRANSACTION_REJECTED`, `SIGNER_UNAVAILABLE`, `PERMISSION_REVOKED` |

**Direct errors and Operation errors.** For a method that returns an Operation, a failure of checks 1 to 9 of §8 MUST be returned as a direct `PwcError`, and no Operation is created. Later failures MAY be returned either directly, before the Operation is created, or as a `failed` Operation. Clients MUST therefore handle every error code in both positions.

### 9.2 Approval invariants

A User approval MUST bind to the authenticated Client, wallet/account, network, complete PSBT or message bytes, relevant transaction details, authorized signing inputs and request expiration. Changing any security-relevant field after approval requires fresh approval. UI MUST derive transaction outputs, destinations, fees and sighash flags from the actual PSBT; the Provider MUST refuse if required information cannot be verified safely. Merely opening or unlocking the wallet is not transaction approval. Hardware devices remain an additional independent security boundary.

### 9.3 Idempotency and concurrency

All potentially mutating, signing or approval-creating methods defined above require `idempotency_key`, a Client-chosen `Id` unique per intended operation. The Provider MUST scope it to the authenticated Client and method. The following rules apply:

- An exact retry MUST produce the same logical response, including the original `operation_id` if the method returns an Operation, and MUST NOT create a second approval, fresh receive address or transaction. It is answered at check 5 of §8, before the scope checks.
- Reuse of a key with different parameters MUST return `IDEMPOTENCY_CONFLICT`. Clients MUST NOT reuse a key for a different deliberate action.
- A request that fails with a direct `PwcError` creates no idempotency record; a retry with the same key is evaluated afresh.
- For a method that returns an Operation, the idempotency record lives exactly as long as the Operation is retained (§8.5). For other methods it MUST be kept for at least `limits.operation_retention_seconds`. Records SHOULD survive a process restart. After a record is discarded, the key is treated as new.
- A request that would raise the Client's number of nonterminal Operations above `limits.max_concurrent_operations` MUST fail with `RATE_LIMITED`.

Operations that compete for the same UTXO MUST be serialized or rejected consistently; a lock in one Client MUST protect against another Client racing the same wallet.

## 10. Standard errors

A `PwcError` has `{ "code": string, "message": string, "details"?: object, "retryable"?: boolean }`. `message` is human-readable and non-normative; Clients MUST branch on `code`. Sensitive details MUST be redacted. Error names are stable tokens independent of HTTP status or JSON-RPC numeric error codes.

| Code | Meaning |
|---|---|
| `INVALID_ARGUMENT` | Missing required parameter, malformed field, value outside its type's syntax or range, invalid binary encoding, or a cursor used outside its context |
| `PAYLOAD_TOO_LARGE` | A PSBT, raw transaction, message or list exceeds an advertised limit of §4.1 |
| `UNSUPPORTED_METHOD` | Method not implemented by this Provider |
| `UNSUPPORTED_OPERATION` | Method exists but is not supported for this account, input, format or state |
| `UNSUPPORTED_PSBT_VERSION` | PSBT is not version 0 |
| `UNSUPPORTED_SCRIPT` | Script type or sighash mode not supported |
| `UNAUTHENTICATED` | No valid authenticated session/principal |
| `PERMISSION_REQUIRED` | No active grant of this Client holds the required scope |
| `PERMISSION_REVOKED` | No active grant holds the required scope, but a retained revoked or expired grant of this Client did |
| `GRANT_NOT_FOUND` | No such grant belonging to this Client |
| `GRANT_CONSTRAINT_VIOLATED` | Request exceeds a constraint of the grant, such as `max_amount_sats_per_request` |
| `USER_REJECTED` | User denied request |
| `WALLET_LOCKED` | Wallet is locked; its data cannot be read until the User unlocks it |
| `WALLET_NOT_FOUND` | No such wallet visible to this Client (nonexistent or outside its grants) |
| `ACCOUNT_NOT_FOUND` | No such account visible to this Client (nonexistent or outside its grants) |
| `NETWORK_MISMATCH` | An address is encoded for a network other than the selected account's, or than the stated `network` |
| `NETWORK_UNAVAILABLE` | Selected network/backend unavailable |
| `SYNC_INCOMPLETE` | Wallet has not completed the synchronization the action needs |
| `STATE_UNAVAILABLE` | Required balance, fee, UTXO or confirmation data cannot be made coherent |
| `SIGNER_UNAVAILABLE` | Required hardware/external signer unavailable |
| `INSUFFICIENT_FUNDS` | Insufficient eligible value after fees/policy |
| `POLICY_VIOLATION` | Wallet/user risk policy denies action |
| `PSBT_VALIDATION_FAILED` | Invalid, inconsistent or unreviewable PSBT |
| `PSBT_CONFLICT` | PSBTs of different transactions, or conflicting values for the same key, during combination |
| `TRANSACTION_REJECTED` | Broadcast backend refused transaction |
| `TRANSACTION_NOT_FOUND` | The account has no record of the requested transaction |
| `RESERVATION_NOT_FOUND` | No such reservation of this Client for this account |
| `OPERATION_NOT_FOUND` | Unknown, no longer retained or unauthorized operation reference |
| `OPERATION_NOT_CANCELABLE` | Operation already past an irreversible step |
| `OPERATION_EXPIRED` | Approval or operation window expired |
| `OPERATION_CANCELED` | Operation was canceled before an irreversible step |
| `IDEMPOTENCY_CONFLICT` | Same key used with different parameters |
| `CURSOR_EXPIRED` | Listing cursor cannot be continued safely |
| `RATE_LIMITED` | Client exceeds a request quota or `max_concurrent_operations` |
| `PROVIDER_UNAVAILABLE` | The Provider cannot currently be reached. Generated by a transport component, such as a browser bridge or a relay client, never by the Provider itself |
| `INTERNAL_ERROR` | Unexpected Provider error; without sensitive diagnostics |

Providers MUST NOT convert an authorization error into an empty-wallet success result. Errors in method execution MUST NOT be hidden by returning success with zero balances or with missing or fabricated signature data. An empty `signed_input_indexes` under the condition of §4.7 is a genuine success, not a hidden error.

## 11. Versioning and extensibility

- PWC Core uses semantic versions `MAJOR.MINOR.PATCH` (Semantic Versioning 2.0.0) for stable releases: incompatible changes increment `MAJOR`, backward-compatible features increment `MINOR`, and corrections increment `PATCH`. Pre-1.0 review drafts may make incompatible changes between any two revisions and MUST NOT be mistaken for deployed stable versions.
- Providers MUST advertise `pwc_version`, `profiles`, supported `methods`, `features` and `scopes`. Consumers MUST negotiate capabilities; platform name is not a capability.
- A Provider claiming a Core version MUST implement every mandatory method in §5.1 with the stated method and data semantics. An unsupported account-level operation MAY return the specified error.
- Extension methods, features, scopes, event types and error codes are named `vendor.<reverse-domain>.<name>`, for example `vendor.com.example.exportLabels`. The `category.action` form of §3.1 is reserved for names registered by PWC specifications, that is Core and the transport profiles of §15. Extensions MUST NOT reuse defined Core names with changed semantics.
- Unknown optional response fields are ignored; unknown *feature names* MUST NOT be interpreted as grants. New mandatory fields require appropriate versioning.
- A PWC transport profile MUST state precisely how methods, successes, errors, identity proofs, grants and event delivery map into its own message format, and names the envelope it uses. PWC-TRANSPORT-LOCAL uses JSON-RPC 2.0.

## 12. Security and privacy requirements

1. **Noncustodial boundary:** PWC MUST NOT expose mnemonic phrases, root secrets, plaintext private keys, raw signing nonces, wallet passwords or unrestricted key derivation. Those operations are expressly out of scope.
2. **Least privilege:** wallet/account inventories, balances, UTXO details, address history, descriptors and transaction histories are sensitive; disclosure requires appropriate grants. Data of a locked wallet is not disclosed at all (§8).
3. **Actual transaction inspection:** User-facing approval MUST be computed from parsed transaction data. A Client-provided title, image, address label or claimed amount has no authority.
4. **Explicit signing:** `psbt:sign` and `messages:sign` authorize *requesting* a signature; each specific signature requires its own local approval. This revision does not standardize unattended signing.
5. **Separation of steps:** creating, signing, combining, finalizing and broadcasting are separate methods. Pure PSBT combination/finalization confers no permission to sign or broadcast. No step silently performs the next one.
6. **Hardware signer integrity:** host UI MUST NOT claim a hardware confirmation occurred merely because an API result returned. If a hardware signer is involved, the Provider MUST preserve its device-confirmation path.
7. **Network and account consistency:** methods MUST use the wallet's existing native network/profile and selected account. An address encoded for another network MUST be rejected with `NETWORK_MISMATCH`; PSBT inputs from another network are inputs the account cannot sign (§8.3). No new Purity chain identity is introduced by PWC.
8. **No blind trust in backends:** uncertain chain state, fees or unverified UTXO ownership MUST lead to marked stale data, deferral or refusal, not fabricated balances, confirmations or spendability.
9. **Channel requirements:** later transport profiles MUST establish an authenticated, confidential, replay-resistant binding appropriate to their environment and MUST defend against cross-origin, local-process, relay and callback attacks as relevant.
10. **Privacy hygiene:** event metadata, logs, diagnostics and permission prompts MUST avoid leaking PSBTs, account contents, connection secrets or user identities to other Clients or unauthenticated parties. PSBTs returned to a Client carry only the data permitted by §4.7.
11. **Resource limits:** Providers MUST enforce bounded input sizes, parsing depth, per-Client concurrency and timeouts. Oversized PSBTs or message floods MUST fail safely.
12. **Audit and revocation:** the Provider MUST offer a User-visible way to inspect/revoke third-party grants and SHOULD log sensitive grants, signing attempts and broadcast decisions without persisting secrets or unnecessary PSBT contents.
13. **Prompt flooding:** the Provider MUST rate-limit, per Client, requests that would show an approval prompt, and refuse requests above that rate with `RATE_LIMITED`. After the User rejects a request, the Provider SHOULD offer to block further prompts from that Client.

## 13. Conformance and test requirements

A conforming Provider MUST advertise its profile(s) and methods in `Capabilities`, support `pwc-core-read-v1`, and MUST obey Core authorization and type semantics. Capability-gated features are tested only if advertised. A transport adapter MUST preserve Core semantics when one is later defined.

| ID | Test | Required outcome |
|---|---|---|
| C-01 | Client without a grant enumerates wallets | `PERMISSION_REQUIRED`, no inventory leak |
| C-02 | Approved Client reads an account | Only approved account data |
| C-03 | Grant revoked during pending signing | No subsequent unauthorized signing; Operation `failed` with `PERMISSION_REVOKED` |
| C-04 | Watch-only account asked to sign | `UNSUPPORTED_OPERATION` |
| C-05 | Retry with identical idempotency key/params | Same logical result or operation ID |
| C-06 | Reuse idempotency key with changed PSBT | `IDEMPOTENCY_CONFLICT` |
| C-07 | Malformed/inconsistent PSBT input values | `PSBT_VALIDATION_FAILED`, no blind signing |
| C-08 | PSBT with fee rate above `limits.high_fee_rate`, or with a nondefault sighash mode | `POLICY_VIOLATION`, or approval prompt carrying the separate warning |
| C-09 | Partial PSBT signing | Returned PSBT preserves the transaction; no implicit finalization/broadcast |
| C-10 | Requested input subset `[0]` | No signing of any other input |
| C-11 | Two complementary same-transaction PSBTs | `psbt.combine` returns one merged, unfinalized PSBT |
| C-12 | Two PSBTs of different transactions | `PSBT_CONFLICT`, no silent override |
| C-13 | PSBT with `PSBT_GLOBAL_VERSION` 2 sent to `psbt.sign`, `psbt.combine` or `psbt.finalize` | `UNSUPPORTED_PSBT_VERSION`, no conversion |
| C-14 | Finalize incomplete PSBT | `complete:false`, no raw transaction or signature creation |
| C-15 | Broadcast accepted by backend | Operation `completed` with `txid`; transaction remains unconfirmed until chain observation |
| C-16 | Reorg moves confirmed transaction | Recomputed state/confirmations |
| C-17 | Unknown feature/response extension | Safe forward-compatible handling |
| C-18 | Cross-Client operation access | `OPERATION_NOT_FOUND`, no result leak |
| C-19 | `psbt.create` output address encoded for another network | `NETWORK_MISMATCH` |
| C-20 | Transport disconnect/retry | No duplicate approval or signing |
| C-21 | Missing external signer | Operation `awaiting_signer`, or `failed` with `SIGNER_UNAVAILABLE` |
| C-22 | Invalid `FeeRate`: `"0"`, `"1.25"`, `"-5"`, `"01250"`, `"+1250"`, JSON number `1250` | `INVALID_ARGUMENT` |
| C-23 | Fee rate `"1250"` at 250 vB | 313-sat nominal minimum, integer rounding |
| C-24 | Pending self-funded spend with change, plus an unconfirmed external receipt | Change in `pending_self_sats`, receipt in `pending_external_sats`; components sum to the signed delta |
| C-25 | Pending replacement/conflict | Exclude superseded branch or return `STATE_UNAVAILABLE`; never sum conflicts |
| C-26 | Malformed `Sats`/`SignedSats`, out-of-range `vout` | `INVALID_ARGUMENT` |
| C-27 | Unauthorized event listener | No protected wallet information disclosed |
| C-28 | UTXO response with internal wallet fields | Response excludes private or unregistered fields |
| C-29 | `Sats` above `2100000000000000` | `INVALID_ARGUMENT` |
| C-30 | PSBT larger than `limits.max_psbt_bytes` | `PAYLOAD_TOO_LARGE` |
| C-31 | Any request on an Operation in a terminal state | State, `result` and `error` never change |
| C-32 | `psbt.sign` whose net outflow including fee exceeds `max_amount_sats_per_request` | `GRANT_CONSTRAINT_VIOLATED`, no approval prompt |
| C-33 | Grant with `expires_at` later than `constraints.valid_until` | Grant expires at `valid_until` |
| C-34 | Wallet listed in `wallet_ids` gains a new account | New account covered by the grant |
| C-35 | Read methods on a locked wallet | `WALLET_LOCKED`; `wallet.list` shows `state:"locked"` and empty `account_ids` |
| C-36 | `account_id` outside the grant versus nonexistent `account_id` | Identical `ACCOUNT_NOT_FOUND` responses |
| C-37 | Pre-grant `session.requestPermissions` Operation | Pollable by its requester only; no wallet/account revealed before approval |
| C-38 | Cursor reused with another account | `INVALID_ARGUMENT` |
| C-39 | PSBT returned by `psbt.create` | No data about UTXOs, addresses or accounts outside the transaction |
| C-40 | Finalize a fully signed PSBT | `complete:true`; `txid` matches `raw_transaction` |
| C-41 | `input_indexes` empty, duplicated or out of range | `INVALID_ARGUMENT` |
| C-42 | `input_indexes` naming an input the account cannot sign | `UNSUPPORTED_OPERATION`, nothing signed |
| C-43 | `psbt.sign` on inputs the account already signed | Success with empty `signed_input_indexes` |
| C-44 | Broadcast refused by backend | Operation `failed` with `TRANSACTION_REJECTED` |
| C-45 | `message.sign` of a message with non-ASCII and trailing whitespace | Signature verifies over the exact UTF-8 bytes |
| C-46 | Inputs of a reserved PSBT | Reported as `locked` until the reservation ends; selectable after `reserved_until` |
| C-47 | `psbt.releaseReservation` called twice | `released:true`, then `released:false`; inputs selectable again |
| C-48 | `psbt.releaseReservation` with another Client's `reservation_id` | `RESERVATION_NOT_FOUND` |
| C-49 | User spends a reserved output in the wallet's own interface | Spend succeeds; reservation ends |
| C-50 | `session.revokePermissions` with another Client's `grant_id` | `GRANT_NOT_FOUND` |
| C-51 | Request beyond `limits.max_concurrent_operations` | `RATE_LIMITED`, no Operation created |
| C-52 | Approved permission request | Granted scopes, resources, expiry and constraints never exceed the request |
| C-53 | Wallet locked or unavailable | No `account.*` or `wallet.syncChanged` events for that wallet |
| C-54 | Broadcast of a transaction the backend already holds | Operation `completed` with its `txid` |
| C-55 | Same-key signatures that differ and both verify | Merged; the value of the first submitted PSBT is kept |
| C-56 | Operation still awaiting approval at its `expires_at` | State `expired` with `OPERATION_EXPIRED` |
| C-57 | `transaction.get` for an unknown txid | `TRANSACTION_NOT_FOUND` |
| C-58 | Read methods on a wallet in state `unavailable` | `STATE_UNAVAILABLE` |
| C-59 | Exact retry after the grant is revoked | Original Operation returned, without `result` |
| C-60 | `message.verify` with a network not in `supported_networks` | `UNSUPPORTED_OPERATION` |
| C-61 | Transaction paying the account more than it spends, under `max_amount_sats_per_request` | Net outflow counts as zero; limit not triggered |
| C-62 | Repeated approval-creating requests after a User rejection | Refused with `RATE_LIMITED` above the Provider's rate |
| C-63 | `psbt.sign` on a PSBT whose inputs come from another network | `UNSUPPORTED_OPERATION`, nothing signed |
| C-64 | `operation.get` of a completed `psbt.create` after its wallet locks | Operation returned without `result` |

Implementations SHOULD use isolated test fixtures for signing and broadcast. Wallet behavior on actual networks is outside a static JSON Schema test; conformance requires both schema validation and behavioral assertions.

## 14. Informative examples (transport-neutral)

The following are *method + params / result* objects, not protocol transport envelopes. Dates, account IDs, PSBTs and block heights are fabricated examples. Placeholders such as `<base64-psbt>` are not valid wire values and are shown only for clarity.

### 14.1 Request consent

```json
{"method":"session.requestPermissions","params":{"scopes":["wallets:read","balances:read","psbt:create","psbt:sign"],"idempotency_key":"grant-001"}}
```

```json
{"operation_id":"op-1001","method":"session.requestPermissions","state":"awaiting_approval","created_at":"2026-11-14T00:00:00Z","updated_at":"2026-11-14T00:00:00Z","expires_at":"2026-11-14T00:10:00Z"}
```

### 14.2 Balance including unconfirmed change

```json
{"method":"account.getBalance","params":{"account_id":"acct-01"}}
```

```json
{
  "account_id":"acct-01","network":"mainnet",
  "confirmed_sats":"1000000","pending_self_sats":"99000","pending_external_sats":"0",
  "pending_outgoing_sats":"400000","unconfirmed_delta_sats":"-301000",
  "spendable_sats":"600000","locked_sats":"0","immature_sats":"0",
  "updated_at":"2026-11-14T00:01:00Z",
  "sync":{"state":"synced","tip_height":971000}
}
```

The effective value is 699,000 sat; the Provider in this example does not select unconfirmed change, so `spendable_sats` is 600,000 sat.

### 14.3 Create PSBT with integer fee rate

```json
{"method":"psbt.create","params":{"account_id":"acct-01","outputs":[{"address":"<existing-purity-address>","amount_sats":"50000"}],"fee_policy":{"type":"rate","sat_per_kvb":"1250"},"idempotency_key":"create-001"}}
```

The `sat_per_kvb` value above is **1.25 sat/vB**, represented without any fractional JSON value. The completed Operation's `result` may be:

```json
{"account_id":"acct-01","network":"mainnet","psbt":"<base64-psbt>","psbt_version":0,"fee_sats":"177","vsize":141,"change_output_indexes":[1]}
```

At 141 vB the nominal minimum fee is `ceil(1250 × 141 / 1000) = 177` sat.

### 14.4 Sign partially, then combine

```json
{"method":"psbt.sign","params":{"account_id":"acct-01","psbt":"<base64-psbt>","input_indexes":[0],"idempotency_key":"sign-001"}}
```

The completed signing operation's `result` may be:

```json
{"account_id":"acct-01","network":"mainnet","psbt":"<updated-base64-psbt>","psbt_version":0,"signed_input_indexes":[0],"signing_status":"partial"}
```

A Client may combine independent signing outputs without selecting an account:

```json
{"method":"psbt.combine","params":{"psbts":["<partial-A>","<partial-B>"]}}
```

```json
{"psbt":"<merged-base64-psbt>","psbt_version":0}
```

`psbt.combine` does not finalize or broadcast. A subsequent `psbt.finalize` is a separate data operation.

## 15. Specification family

PWC Core is one document of a family. The other documents build on it without changing its semantics:

| Document | Purpose |
|---|---|
| PWC-TRANSPORT-LOCAL-001 | Native Clients on the same computer and operating-system user: operating-system local sockets, JSON-RPC 2.0 framing, pairing and mutual authentication, delegated principals for browser bridges. No network listener |
| PWC-TRANSPORT-BROWSER-001 | Web pages through a browser extension: provider discovery, the page interface, the web origin as principal; the extension is the Provider or bridges to a LOCAL Provider |
| PWC-TRANSPORT-REMOTE-001 | Client and Provider on different devices: Nostr relays, NIP-44 encryption, QR pairing with SAS confirmation, message splitting, replay protection |
| PWC-TRANSPORT-MOBILE-001 | Applications on the same phone: pairing, wake and return links; messages over REMOTE |
| PWC-CONFORMANCE-001 | Conformance classes, the test kit, including the companion JSON Schema of §3.2, and the report format |
| PWC-REGISTRY-001 | Every name and identifier the family defines |
| PWC-GOVERNANCE-001 | License, roles, change process and document status |
| PWC-SECURITY-001, PWC-UX-001, PWC-CLIENT-GUIDE-001 | Informative: threat model, approval interface guidelines, Client developer guide |
| PWC-OVERVIEW-001 | Informative map of the family |

## 16. References

- RFC 2119 / RFC 8174, normative terms: https://www.rfc-editor.org/rfc/rfc2119 and https://www.rfc-editor.org/rfc/rfc8174
- RFC 8259, JSON: https://www.rfc-editor.org/rfc/rfc8259
- RFC 3339, timestamps: https://www.rfc-editor.org/rfc/rfc3339
- RFC 4648, base64: https://www.rfc-editor.org/rfc/rfc4648
- BIP-32, hierarchical deterministic wallets: https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki
- BIP-125, opt-in replace-by-fee signaling: https://github.com/bitcoin/bips/blob/master/bip-0125.mediawiki
- BIP-137, message signatures with Bitcoin private keys: https://github.com/bitcoin/bips/blob/master/bip-0137.mediawiki
- BIP-141, segregated witness, weight and virtual size: https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki
- BIP-174, PSBT: https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki
- BIP-322, generic message signing: https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki
- BIP-371, Taproot PSBT fields: https://github.com/bitcoin/bips/blob/master/bip-0371.mediawiki
- Semantic Versioning 2.0.0: https://semver.org/spec/v2.0.0.html
- JSON Schema 2020-12: https://json-schema.org/draft/2020-12

## Appendix A. Remaining design decisions

The following do not re-open the decisions adopted in §0.2 and §0.3. They concern subsequent stabilization or governance:

| ID | Subject | Current draft policy |
|---|---|---|
| D-01 | Mandatory signing profile | Read-only Core required, signing feature optional |
| D-02 | High-level `transaction.send` | Not defined; no hidden create/sign/broadcast |
| D-03 | Unattended signing | Excluded from this Core version |
| D-04 | Portable wallet/account identifiers | Not guaranteed across Provider installations |
| D-05 | Message signing | Optional; BIP-322 forms subject to capability |
| D-06 | New receiving addresses | `address.create` optional |
| D-07 | Transport protocols | Defined by the transport profiles of §15, all review drafts |
| D-08 | Location of the companion JSON Schema | Distributed as `pwc-core-0.2.1.schema.json` with the test kit (PWC-CONFORMANCE-001 §3); no permanent URI assigned (PWC-GOVERNANCE-001, G-03) |

## Appendix B. Change history

### B.1 Changes from v0.2.0

- **Errors:** added `NETWORK_MISMATCH`, `PAYLOAD_TOO_LARGE`, `GRANT_CONSTRAINT_VIOLATED`, `GRANT_NOT_FOUND`, `RESERVATION_NOT_FOUND`, `TRANSACTION_NOT_FOUND`, `OPERATION_CANCELED` and the transport-level `PROVIDER_UNAVAILABLE`; removed the unreferenced `USER_INTERACTION_REQUIRED`; limited `NETWORK_MISMATCH` to address encodings; defined the order of checks (§8), including quota, idempotency and concurrency, the error of every terminal Operation state (§9.1), and identical not-found errors for nonexistent and unauthorized resources.
- **Broadcast:** a refused broadcast is a `failed` Operation with `TRANSACTION_REJECTED`; removed `accepted` from the result.
- **PSBT:** removed PSBT version 2: only version 0 is accepted, any other version gives `UNSUPPORTED_PSBT_VERSION`, and the `psbt_v0`/`psbt_v2` features and the `psbt_version` parameter of `psbt.create` were removed; `psbt.create` returns `fee_sats`, `vsize`, `change_output_indexes`, `reservation_id` and `reserved_until`; added `psbt.releaseReservation`; reservations never block the User's own spending; combination keeps the first of differing valid signatures; defined the order of `psbt.sign` checks, reservation lifetime, PSBT-embedded disclosure limits, `input_indexes` edge cases, the empty-`signed_input_indexes` case, the high-fee warning and the meaning of `complete:true`; `FinalizePsbtResult` always carries the PSBT and its version.
- **Balance:** replaced `pending_incoming_sats` with `pending_self_sats` and `pending_external_sats`; `immature_sats` is required; added the partition rule for `spendable_sats`/`locked_sats`/`immature_sats` and the stale-versus-incoherent rule.
- **Grants:** added `network`; defined expiry as the earlier of `expires_at` and `valid_until`, the fee-inclusive amount limit, and wallet-wide coverage of `wallet_ids`; permission requests can carry `network`, `expires_at` and `constraints`; defined `revoked:false`; a negative net outflow counts as zero; `Grant.account_ids` does not depend on wallet state.
- **Wallet states:** account data of a locked wallet cannot be read; defined the `syncing` and `unavailable` states (§4.2, §8).
- **Capabilities:** added `profiles`, `scopes` (replacing `optional_scopes`), `script_types`, `message_formats` and five limits; removed the `bip322` feature and the pre-grant subset; `Capabilities` is now free of User data.
- **Types:** bounded `Sats`, `FeeRate`, `Id` and `BlockHeight`; restricted `Id` characters; added `Text` for display strings; defined `Network` names (`mainnet`, `signet`, `regtest`), `ScriptType` and `MessageFormat`; removed every `null`; all method parameters use semantic types; `AccountSummary` uses `script_types` and `methods`; removed `AddressRecord.derivation_index` and the `watch_only` UTXO restriction; `address.getReceiveAddress` and `address.create` return `AddressRecord` directly.
- **Messages:** defined the signed bytes, the size limit and the `legacy` format; `message.verify` takes `network` and needs an authenticated session but no scope; an unsupported `network` gives `UNSUPPORTED_OPERATION`.
- **Operations and events:** retention is advertised in `limits.operation_retention_seconds`; idempotency records follow Operation retention; waiting Operations carry a required `expires_at`; defined event payloads; split `grant.revoked` and `grant.expired`.
- **Security:** approval prompts are rate-limited per Client (§12).
- **Family:** replaced the list of future documents (§15) with the specification family; licensed the text under CC BY 4.0 (PWC-GOVERNANCE-001 §2).
- **Editorial:** defined *Provider*; updated the definition of *Grant*; stated that profile-name suffixes are profile revisions; normalized requirement keywords to capitals; corrected the method-name rule and the extension naming; replaced BIP links with the canonical repository and added BIP-32, BIP-125, BIP-137, BIP-141 and Semantic Versioning; extended the conformance table to C-64.

### B.2 Changes from v0.1.0 (in v0.2.0)

- Restricted the normative scope to native on-chain wallet operations; removed future application-layer extension planning.
- Changed `FeeRate` from fractional sat/vB to **canonical positive integer sat/kvB**, replaced `sat_per_vbyte` with `sat_per_kvb`, and specified exact integer rounding.
- Rewrote resource examples to use named semantic types; added the reusable `OutPoint` and explicit PSBT result shapes.
- Replaced `unconfirmed_sats` with required `pending_incoming_sats`, `pending_outgoing_sats` and `unconfirmed_delta_sats`; specified conflict/replacement/change handling.
- Required partial signing, renamed `is_fully_signed` to `signing_status`, and distinguished signature completeness from finalization.
- Added synchronous `psbt.combine` and made `psbt.finalize` account-independent and synchronous; neither creates signatures, accesses wallet keys or broadcasts.
- Aligned the permission matrix, error codes, conformance tests, examples, companion JSON Schema and method registry.

---

**End of PWC-CORE-001 v0.2.1 English review draft.**
