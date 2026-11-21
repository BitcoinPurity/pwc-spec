# Purity Wallet Connect (PWC)
## Approval Interface Guidelines

**Specification:** PWC-UX-001  
**Version:** 0.1.0 (review draft)  
**Date:** 2026-11-21  
**Language:** English  
**License:** CC BY 4.0  
**Status:** *Informative.* Requirements quoted with "MUST" are restatements of the normative sections cited; everything else is advice.

> **Scope rule:** PWC's protection against a malicious Client is, in the end, the User reading a prompt (PWC-SECURITY-001 §5, assumption 4). This guide describes what each prompt should show and how, so that Providers make that reading easy and reliable.

## 1. Principles

1. **Show what will happen, computed by the wallet.** Every amount, address, fee and warning comes from data the Provider parsed and verified (Core §9.2). Client-supplied titles, labels and descriptions are shown, if at all, in a separate area marked "provided by the application".
2. **Name the requester the way the transport proves it.** Show the origin for BROWSER; the paired name, labeled as claimed, plus the operating-system-reported application for LOCAL; the paired name, labeled as claimed, for REMOTE and MOBILE.
3. **One decision per prompt.** A prompt asks one question: allow this access, or sign this transaction. Never combine a permission grant and a signature.
4. **No default to approve.** The approve button is not focused by default, and pressing Enter does not approve.
5. **Unlocking is not approving.** If the wallet must be unlocked first, unlock and approval are separate steps with separate screens (Core §9.2).
6. **Exact values.** Amounts in approval prompts are never rounded.

## 2. Showing the requester

| Transport | Primary line | Secondary line | Never |
|---|---|---|---|
| BROWSER | The origin, registrable domain in bold, e.g. `https://app.**example.com**` | "Private window" if applicable; claimed site name, marked as claimed | The page title or icon as the main identity |
| BROWSER via bridge | The origin, as above | "via ⟨bridge name⟩" | Hiding that a bridge forwarded it |
| LOCAL | The application as reported by the operating system, e.g. its executable path and signer | The claimed name, marked "name given by the application" | The claimed name alone |
| REMOTE, MOBILE | The paired name, marked as claimed, and the pairing date | "Paired from another device" | Any suggestion that the name is verified |

## 3. Permission prompt (`session.requestPermissions`)

Show:

- **Requester** (§2).
- **Network**, prominently if it is not `mainnet`.
- **Wallets and accounts** that will be shared. Let the User narrow the selection; never widen it beyond the request (Core §8.1).
- **Each scope** in plain language with its sensitivity, using the wording below or an equivalent.
- **Expiry** and **amount limit**, if requested; let the User shorten or tighten them.
- For `psbt:create`: that created transactions reveal which coins are used (Core §6).

| Scope | Suggested wording | Sensitivity |
|---|---|---|
| `wallets:read` | See the names of the selected wallets and accounts | Low |
| `balances:read` | See balances of the selected accounts | Medium |
| `addresses:read` | See existing receive addresses | Medium |
| `addresses:create` | Create new receive addresses | Low |
| `transactions:read` | See transaction history | High |
| `utxos:read` | See individual coins and their details | High |
| `psbt:create` | Prepare unsigned transactions; this reveals the coins used | High |
| `psbt:sign` | Ask you to sign transactions; you approve each one | Highest |
| `transactions:broadcast` | Send signed transactions to the network | Medium |
| `messages:sign` | Ask you to sign messages; you approve each one | High |
| `events:read` | Be notified of changes to what it can already see | Low |

## 4. Transaction signing prompt (`psbt.sign`)

Order of content, top to bottom:

1. **Requester** (§2) and **network**.
2. **What leaves the account**: each external output as address and exact amount. Addresses in full, grouped in blocks of four characters for comparison.
3. **Fee**: exact amount in satoshis and BTC, and the rate in sat/vB. Convert from `FeeRate` by dividing sat/kvB by 1,000 and show up to three decimals; for example `"1250"` is 1.25 sat/vB.
4. **Total leaving the account**: external outputs plus fee, the same quantity that `max_amount_sats_per_request` measures (Core §4.8).
5. **Change**: outputs returning to the account, shown collapsed and identified as change.
6. **Which inputs this approval signs**, if it is not all of them (Core §8.3).
7. **Warnings**, each requiring its own acknowledgment (§6).
8. A note that **signing does not send** the transaction.

Amounts appear as `0.00050000 BTC (50,000 sat)`, with eight decimals always shown in the BTC figure.

## 5. Message signing prompt (`message.sign`)

- Show the **exact** message, including leading and trailing whitespace and line breaks, in a monospaced box. Mark characters that are invisible or easily confused, such as zero-width or right-to-left marks.
- Show the **address** in full and the **format**.
- State that signing a message does not move funds but may be used to prove control of the address.

## 6. Warnings

| Condition | Source | Suggested wording |
|---|---|---|
| Fee rate above `limits.high_fee_rate` | Core §8.3 | "The fee is unusually high: X sat/vB, Y sat in total." |
| Fee cannot be determined | Core §8.3 | "The wallet cannot verify the fee of this transaction." |
| Sighash other than the default | Core §8.3 | "This signature lets others change parts of the transaction afterwards." |
| Test network | Core §3.2 | "This is a test network. Coins here have no value." |
| Private browsing principal | BROWSER §6 | "This request comes from a private window." |
| Origin resembling a granted origin | BROWSER §9 | "This site's address resembles ⟨granted origin⟩." |

A warning is acknowledged by a separate action, such as a checkbox, before the approve button becomes active. A warning is never hidden behind a "details" link.

## 7. Pairing prompts

- **LOCAL** (LOCAL §5.3): show the six-digit code large and ask the User to check that the application shows the same code. A delegator pairing (LOCAL §5.8) gets its own stronger wording: "This application will act for websites. The wallet will trust the website addresses it reports."
- **REMOTE and MOBILE** (REMOTE §4.4): show the SAS large on both devices, and keep it visible until the User confirms. The Client asks "Does your wallet show 084 418?" with "Yes" and "No" buttons, never a single "Continue".
- **BROWSER**: there is no pairing; the first permission prompt is the first contact.

## 8. Preventing approval fatigue

- After a rejection, offer "Block requests from this application" (Core §12 item 13).
- Never queue several signing prompts behind one another without returning to the wallet's home screen in between.
- Do not offer "Approve all" or "Remember this decision" for signing. Unattended signing is excluded (Core §0.2).
- Show a quiet indicator, not a prompt, for operations that need no approval, such as reads within a grant.

## 9. Managing connections

Provide a screen that lists, for each Client:

- the requester identity (§2), pairing date and last activity;
- its grants with scopes, accounts, expiry and limits, each revocable;
- for delegators, every delegated origin, each revocable (LOCAL §5.8);
- a button to unpair, which revokes everything (LOCAL §5.7, REMOTE §6).

## 10. Locked wallets and hardware signers

- When a request arrives for a locked wallet, first show "⟨requester⟩ needs the wallet unlocked", then the unlock screen, then the prompt. The prompt never appears before unlocking (Core §8).
- With a hardware signer, show "Confirm on your device" and the values the User should check on the device. Never show "Confirmed" before the device has actually confirmed (Core §12 item 6).

## 11. References

- PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001, -BROWSER-001, -REMOTE-001 and -MOBILE-001 v0.1.0
- PWC-SECURITY-001 v0.1.0

---

**End of PWC-UX-001 v0.1.0.**
