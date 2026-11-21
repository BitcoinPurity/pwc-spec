# Purity Wallet Connect (PWC)
## Security and Threat Model

**Specification:** PWC-SECURITY-001  
**Version:** 0.1.0 (review draft)  
**Date:** 2026-11-21  
**Language:** English  
**License:** CC BY 4.0  
**Covers:** PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001, -BROWSER-001, -REMOTE-001 and -MOBILE-001 v0.1.0  
**Status:** Proposed; input to the security audit named in each specification's status

> **Scope rule:** This document collects, in one place, what the PWC family protects, against whom, by which requirement, and what risk remains. It adds no requirement of its own: every mitigation cites the normative section that imposes it. Findings that call for a change to a specification are listed in §8 for the next revision.

## 1. Assets

| Asset | Why it matters | Primary protection |
|---|---|---|
| **Spending authority** | A signature moves funds irreversibly | Per-action User approval (Core §9.2); no unattended signing (Core §0.2, §12 item 4) |
| **Holdings privacy** | Balances, UTXOs, addresses and history reveal wealth and behavior | Scoped grants (Core §6); field disclosure rules (Core §3.5); PSBT disclosure limits (Core §4.7) |
| **Integrity of what the User sees** | The User approves what is displayed | Prompts derived from the parsed PSBT, never from Client claims (Core §9.2, §12 item 3) |
| **The User's attention** | Repeated prompts lead to careless approval | Prompt rate limits (Core §12 item 13) |
| **Correct receive addresses** | A false address diverts incoming payments | Authenticated Provider identity per transport (§4) |
| **Availability** | Payments and approvals depend on reaching the wallet | Best effort only; idempotent retries (Core §9.3) |

## 2. Trust boundaries

```text
            trusted                    untrusted channel                 untrusted
  ┌──────────────────────────┐   ┌────────────────────────────┐   ┌──────────────────┐
  │ User ⇄ Provider UI       │   │ OS socket │ browser │ relay│   │ Client           │
  │ Provider ⇄ keys/signer   │◀─▶│ (transport profile)        │◀─▶│ (application,    │
  └──────────────────────────┘   └────────────────────────────┘   │  web page)       │
              │                                                    └──────────────────┘
              ▼ untrusted for correctness
        chain backend
```

- The **User and the Provider's own interface** are the only parties whose decisions authorize anything.
- The **Client** is never trusted: not its name, its icon, its labels, nor its description of a transaction.
- The **transport** authenticates who is talking but is never a source of permission (Core §2).
- The **chain backend** may be wrong or stale. The Provider marks staleness and refuses to fabricate (Core §4.3, §12 item 8).

## 3. Attackers and mitigations

Each row names an attacker, the transports where the attacker is relevant, the mitigation, and the residual risk.

| ID | Attacker | Transports | Mitigation | Residual risk |
|---|---|---|---|---|
| T-01 | Malicious Client holding a grant, trying to obtain a harmful signature | All | Prompt shows outputs, fee and sighash parsed from the PSBT (Core §9.2); high-fee and nondefault-sighash warnings (Core §8.3); amount limit (Core §4.8); input subset enforcement (Core §8.3) | A User who approves without reading |
| T-02 | Malicious Client without a grant, probing for wallet data | All | Capabilities free of User data (Core §4.1); identical not-found errors (Core §8, check 7); no empty-wallet success for authorization errors (Core §10) | None known |
| T-03 | Client impersonating a well-known application | LOCAL, REMOTE, MOBILE | Claimed names marked as unverified; operating-system peer information shown (LOCAL §6); pairing confirmation by code or SAS (LOCAL §5.3, REMOTE §4.4) | Users who trust the claimed name |
| T-04 | Look-alike web origin | BROWSER | Origin shown prominently with registrable domain emphasized (BROWSER §6); look-alike warning (BROWSER §9, SHOULD) | Users who do not read the origin |
| T-05 | Malicious web page or cross-origin frame | BROWSER | Origin determined by the browser (BROWSER §3.1); frames not served (BROWSER §3.2); prompts outside the page (BROWSER §6) | None known |
| T-06 | Network observer or relay operator | REMOTE, MOBILE | NIP-44 encryption, signatures by pinned keys (REMOTE §5.3, §5.4); fresh transport keys (REMOTE §3) | Timing, sizes and IP addresses visible to relays |
| T-07 | Replay or duplication of requests | All | Idempotency keys (Core §9.3); fresh nonces (LOCAL §5.4); event ids and time window (REMOTE §5.5) | None known |
| T-08 | Another operating-system user on the same computer | LOCAL | Private endpoint directory, pipe DACL, peer user check (LOCAL §3) | None known |
| T-09 | Fake Provider: socket or pipe squatting, impostor wallet app, a stranger answering a photographed pairing QR | LOCAL, REMOTE, MOBILE | Mutual HMAC authentication (LOCAL §5.4); server SID check on Windows (LOCAL §3.3); SAS confirmation (REMOTE §4.4, MOBILE §4.1) | First LOCAL pairing against same-user malware (T-11) |
| T-10 | Compromised or wrong chain backend | All | Stale data marked, incoherent data refused (Core §4.3); fees and input amounts validated from PSBT data (Core §8.3) | Wrong balances shown as stale; no theft of funds |
| T-11 | Malicious code running as the same operating-system user | LOCAL, BROWSER bridge | None beyond per-action approval; stated as unprotected (LOCAL §2, BROWSER §2) | Full impersonation of paired Clients |
| T-12 | Script injected into a legitimate origin (XSS, third-party script) | BROWSER | Per-action approval only (BROWSER §9) | Acts with that origin's grants |
| T-13 | Prompt flooding to induce careless approval | All | Per-Client rate limits (Core §12 item 13); pairing prompt limits (LOCAL §5.3); optional user-activation requirement (BROWSER §5.2) | Determined social engineering |
| T-14 | Privacy leakage through PSBT metadata, events or logs | All | PSBT content limited to the transaction (Core §4.7); events no richer than reads (Core §8.6); no secrets or PSBTs in logs (Core §12 item 10, LOCAL §10) | Metadata inherent to the transaction itself |
| T-15 | Resource exhaustion | All | Size limits (Core §4.1, LOCAL §4.1, REMOTE §5.6); concurrency limits (Core §9.3); handshake time limits (LOCAL §5.1) | Relay-level denial of service |
| T-16 | Host interface misrepresenting a hardware signer's confirmation | All | Device confirmation path preserved (Core §12 item 6) | Users who skip checking the device screen |
| T-17 | Malicious delegator claiming arbitrary web origins | BROWSER bridge | Delegator pairing is an explicit high-trust prompt (LOCAL §5.8) | A User who pairs an untrustworthy bridge |
| T-18 | Hostile application hijacking a custom URL scheme | MOBILE | SAS defeats impostor pairing; links carry no data; return only to the bound URL (MOBILE §3, §5) | Users who skip the SAS comparison |

## 4. Security properties by transport

| Property | LOCAL | BROWSER | REMOTE | MOBILE |
|---|---|---|---|---|
| Client authentication | Pairing secret, HMAC per connection | Browser-determined origin | Pinned transport key, signature on every event | As REMOTE |
| Provider authentication | HMAC proof; Windows server SID check | Implicit (the installed extension) | Pinned key; SAS at pairing | As REMOTE |
| Confidentiality | Operating-system access control | Browser process isolation | NIP-44 end-to-end encryption | As REMOTE |
| Replay resistance | Fresh nonces per connection | Document-bound messaging | Event ids plus 300-second window | As REMOTE |
| Network exposure | None | None | Relays over the Internet | Relays over the Internet |
| Same-user malware | Not protected | Not protected | Not protected on either device | Not protected |

## 5. Assumptions

1. The operating system enforces file, pipe and process isolation between users.
2. The browser enforces origin isolation and reports message senders truthfully.
3. secp256k1, SHA-256, HMAC-SHA256, ChaCha20 and HKDF are secure for their use here.
4. The User reads the prompt before approving. Every per-action protection depends on it.
5. Device clocks are within a few minutes of real time, for REMOTE's replay window.
6. The Provider's own code, storage and signing devices are not compromised.

## 6. Explicitly out of scope

| Topic | Reason |
|---|---|
| Malware running as the same operating-system user | No transport can isolate it; stated in each profile's threat model |
| Cross-chain transaction replay between Purity and Bitcoin | A wallet responsibility by project decision; PWC defines no replay checks |
| Seed backup, key generation, wallet storage encryption | Wallet implementation (Core §1.2) |
| Relay availability and censorship | REMOTE does not guarantee delivery (REMOTE §8) |
| Physical access and coercion | Outside any protocol |

## 7. Audit checklist

An auditor of an implementation SHOULD verify at least the following:

1. Every prompt's content comes from parsed data, and no Client-supplied text appears without an "unverified" or "claimed" mark.
2. The order of checks of Core §8 is followed, including identical not-found responses.
3. Operations reach terminal states only as Core §9.1 allows, and results disappear when the wallet locks.
4. Idempotency records are scoped to the Client and method, and survive restarts where claimed.
5. PSBTs returned by `psbt.create` and `psbt.sign` contain no data about other coins or accounts.
6. LOCAL: directory and pipe permissions, the peer check, constant-time proof comparison, no secret in logs.
7. BROWSER: origin taken from `MessageSender`, frames refused, prompts outside the page.
8. REMOTE: signature and pinned-key checks before decryption results are used, the replay window, part limits, fresh keys per pairing.
9. MOBILE: no data in links, return only to the bound URL.
10. The conformance tests of PWC-CONFORMANCE-001 for the implementation's classes pass.

## 8. Findings for the next revision

| ID | Finding | Affected text | Recommendation |
|---|---|---|---|
| F-01 | JSON objects with duplicate member names may be read differently by a bridge and a Provider | Core §3.1 | Require Providers and transport components to reject duplicate member names with `INVALID_ARGUMENT` |
| F-02 | LOCAL Providers must keep Client secrets in recoverable form | LOCAL §7 | Consider an asymmetric Client credential (a signature key) so that the Provider stores only public keys |
| F-03 | The first LOCAL pairing has no defense against a same-user process posing as the Provider | LOCAL §2 | Document as accepted risk, or bind pairing to an operating-system-verified Provider identity |
| F-04 | Look-alike origin detection is only a SHOULD and has no algorithm | BROWSER §9 | Define a minimum check, such as confusable-character and edit-distance tests against granted origins |
| F-05 | REMOTE kind `27180` is unregistered | REMOTE Appendix A | Register it before stabilization |
| F-06 | The companion schemas use the placeholder domain `purity-wallet-connect.invalid` in `$id` | Schemas | Assign a permanent URI before stabilization |

## 9. References

- PWC-CORE-001 v0.2.1
- PWC-TRANSPORT-LOCAL-001, PWC-TRANSPORT-BROWSER-001, PWC-TRANSPORT-REMOTE-001, PWC-TRANSPORT-MOBILE-001, all v0.1.0
- PWC-CONFORMANCE-001 v0.1.0

---

**End of PWC-SECURITY-001 v0.1.0.**
