# Purity Wallet Connect (PWC)
## Transport Profile MOBILE: Applications on the Same Phone

**Specification:** PWC-TRANSPORT-MOBILE-001  
**Version:** 0.1.0 (review draft; not a deployed standard)  
**Date:** 2026-11-21  
**Language:** English (normative draft)  
**License:** CC BY 4.0  
**Requires:** PWC-CORE-001 v0.2.1; PWC-TRANSPORT-REMOTE-001 v0.1.0  
**Status:** Proposed specification, subject to owner review and security audit

> **Scope rule:** This profile covers a Client application and a wallet application on the **same phone or tablet**. Messages travel exactly as in PWC-TRANSPORT-REMOTE. This profile adds only what a mobile operating system needs: pairing by opening a link instead of scanning a QR code, a link that brings the wallet to the foreground, a link that returns the User to the Client, and rules for applications that the system suspends.

## 0. Decisions, status and interpretation

### 0.1 Decisions adopted in v0.1.0

- Every PWC message travels over REMOTE. Links carry no request, result or account data.
- Pairing opens the REMOTE pairing URI as a link; SAS comparison (REMOTE §4.4) remains mandatory.
- The Provider gives the Client a `wake_url` at pairing. The Client opens it to bring the wallet to the foreground before an approval is needed.
- The Client registers a `return` URL at pairing. The Provider returns the User only to that URL.
- Verified links (iOS Universal Links, Android App Links) are preferred over custom URL schemes.

The present version remains a **review draft**. Open questions are listed in Appendix A.

### 0.2 Normative vocabulary

Requirement keywords have the meanings given in PWC-CORE-001 §0.4. "REMOTE §n" points into PWC-TRANSPORT-REMOTE-001 v0.1.0, and "Core §n" into PWC-CORE-001 v0.2.1.

## 1. Scope

### 1.1 Covered

A Client application and a Provider application installed on the same mobile device, both able to reach the Internet.

### 1.2 Why messages use REMOTE

Mobile operating systems offer no general, two-way channel between applications of different developers. Links can only start an application with a short URL. A relay channel works the same on every platform, keeps request content out of URLs and system logs, and reuses all of REMOTE's authentication, encryption and replay protection.

### 1.3 Not covered

Communication through platform-specific mechanisms such as Android bound services, intents with results or iOS app groups. A later version may define them (Appendix A, MA-01).

## 2. Threat model

| Attacker | Protected | Mechanism |
|---|---|---|
| Another application that registers the same custom URL scheme and receives the pairing link | Yes, if the User compares the SAS | REMOTE §4.4. The impostor cannot show the SAS in the real wallet |
| An application that opens the wake link to bring the wallet to the foreground | Harmless | The wake link carries no request; the wallet shows nothing until a signed request arrives over REMOTE (§4) |
| An application that crafts a return link or an open-redirect through the wallet | Yes | The Provider returns only to the URL bound at pairing (§5) |
| Leakage through URLs, which systems may log | Yes | Links carry no request, result or account data (§3) |
| Malicious code on the device with elevated rights | **No** | Out of scope |

## 3. Links

Three kinds of link are used, and none of them may carry anything beyond the parameters listed:

| Link | Opened by | Target | Parameters |
|---|---|---|---|
| Pairing link | Client | Any wallet that handles `pwc-remote:` | The REMOTE pairing URI (REMOTE §4.1), optionally with `return` |
| Wake link | Client | The paired wallet | `wake_url` from pairing, plus `pwc_session` |
| Return link | Provider | The paired Client | `return` from pairing, plus `pwc_session` |

`pwc_session` is the `session_id` of the pairing, percent-encoded. Links MUST NOT carry PSBTs, transactions, messages to sign, addresses, balances or any other request or result data.

## 4. Pairing and waking

### 4.1 Pairing on one device

The Client builds the REMOTE pairing URI. It MAY add `return` (§5), and opens the URI as a link instead of showing a QR code. The following rules apply:

- On Android the system lets the User choose an application if several handle `pwc-remote:`. The Client SHOULD NOT bypass that choice.
- On iOS the system decides which application receives a custom scheme. The SAS comparison is therefore the only defense against an impostor, and the Client MUST NOT treat the pairing as established before the User confirms the SAS (REMOTE §4.4).
- To compare the SAS, the User switches between the two applications. Both SHOULD keep the SAS visible until the User confirms or cancels.
- The Client MAY also show the pairing URI as a QR code, so that a wallet on another device can pair instead (REMOTE).

### 4.2 `wake_url`

A Provider that supports this profile includes `wake_url` in `remote.paired` (REMOTE §4.3). The following rules apply:

- `wake_url` SHOULD be a verified link (an iOS Universal Link or an Android App Link) on a domain controlled by the wallet's publisher. It MAY be a URL with a custom scheme registered by the wallet.
- It MUST NOT contain secrets. It identifies the wallet application, not the pairing.
- To wake the wallet, the Client opens `wake_url` with the parameter `pwc_session` added.

When the wallet opens through its wake link, it connects to its relays and processes incoming requests for that pairing. If `pwc_session` does not name one of its pairings, it ignores the parameter.

### 4.3 When to wake

A suspended wallet is not connected to its relays, and requests sent to it are lost (REMOTE §8). A Client therefore follows this sequence for every request that may need the User's attention, which includes `session.requestPermissions`, `psbt.sign` and `message.sign`:

1. Open the wake link.
2. Call `remote.ping` until it succeeds, for at most 30 seconds.
3. Send the request.
4. If the request receives no response, retry it with the same `idempotency_key` (Core §9.3).

For other requests a Client MAY first send the request and wake the wallet only if `remote.ping` fails.

## 5. Returning to the Client

A Client that wants the User returned after an approval includes `return` in its pairing URI. The following rules apply:

- `return` SHOULD be a verified link of the Client. It MUST be an `https:` URL or a URL with a custom scheme, and MUST NOT contain secrets.
- The Provider stores `return` with the pairing. It never accepts a return address from any other source, such as a request parameter.
- After the User has acted on a prompt that a request of that pairing caused, the Provider MAY open `return` with `pwc_session` added. It MUST NOT open it in any other situation.
- The Client, when opened through its return link, fetches the outcome with `operation.get` over REMOTE. The return link itself says nothing about the outcome.

## 6. Application lifecycle

- A Provider SHOULD stay connected to its relays while it is in the foreground, and MAY keep the connection for as long as the operating system allows in the background.
- A Provider MUST NOT show an approval prompt for a request it received while in the background until the User brings it to the foreground. It MAY post a local notification, generated on the device, that leads the User to the prompt. Remote push delivery is outside Core (Core §1.2).
- A Client MUST be prepared for the wallet to be suspended at any time, and relies on `remote.ping`, the wake link and idempotent retries.
- When the system ends either application, the pairing survives. The pairing state and transport keys are stored as REMOTE §3 requires.

## 7. Mapping of Core obligations

All Core obligations are met by REMOTE (REMOTE §9). This profile adds:

| Concern | Section |
|---|---|
| Pairing without a second device | §4.1 |
| Reaching a suspended Provider | §4.2, §4.3 |
| Returning the User without an open redirect | §5 |
| No request data in URLs (Core §12, item 10) | §3 |

## 8. Conformance tests

| ID | Test | Required outcome |
|---|---|---|
| M-01 | Pairing link opened on the same device | Pairing completes over REMOTE after SAS confirmation |
| M-02 | Pairing link received by an impostor application | SAS mismatch; Client discards the pairing |
| M-03 | Any link built by the Client or Provider | Contains no request, result or account data |
| M-04 | Wake link opened by a third application | Wallet opens; no prompt appears without a signed request |
| M-05 | Client sends `psbt.sign` to a suspended wallet | Client wakes the wallet, `remote.ping` succeeds, request sent once |
| M-06 | Request lost while the wallet was suspended, then retried | One Operation, thanks to the idempotency key |
| M-07 | After the User approves a prompt | Provider opens the bound `return` URL with `pwc_session` |
| M-08 | A request names a different return address | Ignored; the Provider uses only the bound `return` |
| M-09 | Pairing made without `return` | Provider never opens a return link |
| M-10 | Request received while the wallet is in the background | Prompt shown only after the wallet is in the foreground |

## 9. Informative example

```text
1. Client opens:
   pwc-remote://<client-pubkey>?v=1&secret=<32 hex>&relay=wss%3A%2F%2Frelay.example.com&name=Example%20Trader&return=https%3A%2F%2Ftrader.example%2Fpwc-return
2. Wallet shows the SAS; the User approves and switches back; the Client shows the same SAS; the User confirms.
3. remote.paired carried "wake_url":"https://wallet.example/pwc-wake"
4. Before psbt.sign the Client opens https://wallet.example/pwc-wake?pwc_session=ss-41
5. remote.ping succeeds; the Client sends psbt.sign over REMOTE; the User approves in the wallet.
6. The wallet opens https://trader.example/pwc-return?pwc_session=ss-41
7. The Client calls operation.get over REMOTE and receives the signed PSBT.
```

## 10. References

- PWC-CORE-001 v0.2.1; PWC-TRANSPORT-REMOTE-001 v0.1.0
- Apple, supporting Universal Links: https://developer.apple.com/documentation/xcode/supporting-universal-links-in-your-app
- Android, App Links: https://developer.android.com/training/app-links

## Appendix A. Open questions

| ID | Subject | Current draft policy |
|---|---|---|
| MA-01 | Platform-specific channels (Android bound services, iOS app groups) | Not defined; all messages use REMOTE |
| MA-02 | Offline use on one device | Not supported; both applications need relay access |
| MA-03 | A registered default wallet for `pwc-remote:` | Left to the operating system |

---

**End of PWC-TRANSPORT-MOBILE-001 v0.1.0 English review draft.**
