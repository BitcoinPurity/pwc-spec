# Purity Wallet Connect (PWC)
## Specification Family Overview

**Specification:** PWC-OVERVIEW-001  
**Version:** 0.1.0  
**Date:** 2026-11-21  
**Language:** English  
**License:** CC BY 4.0  
**Status:** Informative

> **Scope rule:** One page that lists the PWC documents, how they depend on each other, which ones a product must implement, and in what order to read them.

## 1. What PWC is

Purity Wallet Connect lets applications use a Bitcoin Purity wallet without ever holding its keys. An application asks; the wallet shows the User exactly what would happen; the User decides. **Core** defines the operations and the rules of consent independently of any platform. **Transport profiles** define how messages travel on each platform.

## 2. Documents

| Document | Version | Kind | Purpose |
|---|---|---|---|
| PWC-CORE-001 | 0.2.1 | Normative | Data types, methods, permissions, Operations, errors, events |
| PWC-TRANSPORT-LOCAL-001 | 0.1.0 | Normative | Native applications on the same computer: local sockets, pairing, HMAC authentication, delegation for browser bridges |
| PWC-TRANSPORT-BROWSER-001 | 0.1.0 | Normative | Web pages through a browser extension, either as the wallet itself or as a bridge to a desktop wallet |
| PWC-TRANSPORT-REMOTE-001 | 0.1.0 | Normative | Different devices over Nostr relays: QR pairing, SAS, encryption, splitting |
| PWC-TRANSPORT-MOBILE-001 | 0.1.0 | Normative | Applications on the same phone: pairing, wake and return links over REMOTE |
| PWC-CONFORMANCE-001 | 0.1.0 | Normative | Conformance classes, test kit, report format |
| PWC-REGISTRY-001 | 0.1.0 | Normative | Every defined name and identifier |
| PWC-GOVERNANCE-001 | 0.1.0 | Normative | License, roles, change process, status lifecycle, security reports |
| PWC-SECURITY-001 | 0.1.0 | Informative | Consolidated threat model, audit checklist, findings |
| PWC-UX-001 | 0.1.0 | Informative | What approval and pairing prompts should show |
| PWC-CLIENT-GUIDE-001 | 0.1.0 | Informative | How to build a Client, with tested examples |
| PWC-OVERVIEW-001 | 0.1.0 | Informative | This document |

All are review drafts (PWC-GOVERNANCE-001 §6).

## 3. Dependencies

```text
PWC-CORE-001
├── TRANSPORT-LOCAL
├── TRANSPORT-BROWSER ─── bridge mode uses ────► LOCAL §5.8 (delegated principals)
├── TRANSPORT-REMOTE ──── message format of ───► LOCAL §4.2, §4.3
└── TRANSPORT-MOBILE ──── all messages over ───► REMOTE

REGISTRY, CONFORMANCE, SECURITY, UX and CLIENT-GUIDE refer to all of the above.
```

- REMOTE reuses LOCAL's JSON-RPC message format (LOCAL §4.2, §4.3).
- MOBILE carries all messages over REMOTE.
- BROWSER bridge mode uses LOCAL delegated principals (LOCAL §5.8).

## 4. What each product implements

| Product | Must implement | Conformance classes |
|---|---|---|
| PurityWallet desktop | Core; LOCAL, with delegation if a browser bridge is offered | P-CORE, P-SIGNER, P-LOCAL, P-LOCAL-DELEGATION |
| Browser extension that is a wallet | Core; BROWSER extension mode | X-BROWSER, P-SIGNER |
| Browser extension that bridges to PurityWallet | BROWSER bridge mode; a bridge host that is a LOCAL Client | X-BRIDGE |
| Mobile wallet | Core; REMOTE; MOBILE | P-CORE, P-SIGNER, P-REMOTE, P-MOBILE |
| Desktop application or tool | LOCAL Client; optionally REMOTE Client | C-LOCAL, C-REMOTE |
| Web application | BROWSER page interface; optionally REMOTE Client for phone wallets | C-REMOTE where used |
| Mobile application | MOBILE Client | C-MOBILE |

## 5. Reading order

- **Wallet developers:** Core → the transport profiles you ship → SECURITY → UX → CONFORMANCE.
- **Application developers:** CLIENT-GUIDE → Core §4, §8, §10 → the transport profile you use.
- **Auditors:** SECURITY → Core §8, §9, §12 → each transport's threat model section.

## 6. Companion files

| Folder | Contents |
|---|---|
| `specs/` | The documents of §2 |
| `pwc-schema/` | JSON Schemas for Core and the three transports with their own messages, the registry JSON, generators and the schema check |
| `pwc-testkit/` | Test vectors, reference computations, PSBT fixtures, the LOCAL runner, a mock Provider and a minimal Client |

Details: PWC-CONFORMANCE-001 §3.

## 7. Decisions recorded for this release

| Decision | Where |
|---|---|
| Only PSBT version 0 is supported | Core §4.7 |
| Networks: `mainnet`, `signet`, `regtest` | Core §3.2 |
| Cross-chain replay protection is the wallet's responsibility, not PWC's | PWC-SECURITY-001 §6 |
| LOCAL on Windows uses named pipes | LOCAL §3.3, Appendix A LA-01 |
| REMOTE uses Nostr relays | REMOTE §0.1 |
| BROWSER covers both extension wallets and bridges | BROWSER §1.2 |
| Documents and data under CC BY 4.0 | PWC-GOVERNANCE-001 §2 |

---

**End of PWC-OVERVIEW-001 v0.1.0.**
