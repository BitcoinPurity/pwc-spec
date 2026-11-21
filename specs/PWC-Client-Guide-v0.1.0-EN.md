# Purity Wallet Connect (PWC)
## Client Developer Guide

**Specification:** PWC-CLIENT-GUIDE-001  
**Version:** 0.1.0 (review draft)  
**Date:** 2026-11-21  
**Language:** English  
**License:** CC BY 4.0  
**Status:** *Informative.* Requirements quoted with "MUST" restate the normative sections cited.

> **Scope rule:** This guide shows how to build an application that uses a wallet through PWC: which transport to pick, the order of calls, how to retry safely, how to handle amounts and errors, and how to test. The examples were run against the mock Provider of the PWC test kit.

## 1. Choosing a transport

| Your application | Wallet | Transport |
|---|---|---|
| Desktop application or command-line tool | Desktop wallet on the same computer | LOCAL |
| Web page | Browser extension, or a desktop wallet with a browser bridge | BROWSER |
| Web page or desktop application | Wallet on a phone or another computer | REMOTE |
| Mobile application | Wallet application on the same phone | MOBILE (messages over REMOTE) |

All four carry the same Core methods, results and errors, so application logic above the transport is shared.

## 2. Lifecycle

```text
discover ─► connect / pair ─► pwc.getCapabilities ─► session.requestPermissions ─► poll Operation
                                                                                        │
            ◄── handle revocation, locking, unavailability ◄── use methods within grants ◄┘
```

1. **Discover** the Provider: LOCAL endpoint directory (LOCAL §3.5), `pwc:announceProvider` events (BROWSER §4.1), or show a pairing QR code (REMOTE §4.1).
2. **Connect and pair** once, then authenticate on every connection (LOCAL §5.4). BROWSER needs no pairing. On REMOTE and MOBILE, confirm the SAS with the User (REMOTE §4.4).
3. **Read capabilities.** Use only the methods, features and limits listed there; never infer them from the wallet's name (Core §4.1).
4. **Request permissions**, asking only for the scopes you need. The call returns an Operation; poll `operation.get` until it is terminal. On success its `result.grants` lists what the User granted, which may be less than you asked for (Core §8.1).
5. **Use** methods within the grants. Account-scoped methods take an `account_id` from `account.list`.
6. **Expect change**: grants can be revoked, wallets locked and Providers closed at any time.

## 3. Safe retries

- Every method that creates, signs or changes something takes an `idempotency_key` (Core §9.3). Generate a new key for each **intended action** and reuse it for every retry of that action, including after a reconnect. A UUID satisfies the `Id` syntax.
- An exact retry returns the original response, including the original `operation_id`, so a request that did arrive is never performed twice.
- Reusing a key with different parameters returns `IDEMPOTENCY_CONFLICT`. That is a bug in the Client.
- Poll an Operation no more than once per second, and stop at its `expires_at`. With `events:read`, wait for `operation.stateChanged` instead and poll only as a fallback.

## 4. LOCAL example

The test kit file `example_local_client.py`, using the minimal client `pwc_local_client.py`, pairs on first use, stores the credential readable only by the user, and authenticates on later runs:

```python
client = LocalClient(socket_path)
client.say_hello()
if os.path.exists(cred_file):
    cred = json.load(open(cred_file))
    client.authenticate(cred["client_id"], cred["client_secret"])
else:
    paired = client.pair("Example Trader")             # the wallet shows a prompt with the same code
    cred = {"client_id": paired["client_id"], "client_secret": paired["client_secret"]}
    fd = os.open(cred_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)   # readable by this user only
    with os.fdopen(fd, "w") as f:
        json.dump(cred, f)

key = str(uuid.uuid4())                                 # keep it until the Operation is terminal
op = client.request("session.requestPermissions", {"scopes": ["wallets:read", "balances:read"],
                                                    "idempotency_key": key})
```

Against the mock Provider, the first run prints a pairing code and the second run reconnects with the stored credential. Both end with `permission request: awaiting_approval` and `wallet.list: PERMISSION_REQUIRED`, because the mock never approves permission requests. A production Client SHOULD store the secret in the operating system's credential store instead of a file (LOCAL §7).

## 5. BROWSER example

```javascript
const found = [];
window.addEventListener("pwc:announceProvider", (e) => found.push(e.detail));
window.dispatchEvent(new Event("pwc:requestProvider"));
// Let the User choose from `found`; render info.icon only through an <img> element.
const { provider } = found[0];

const op = await provider.request({
  method: "session.requestPermissions",
  params: { scopes: ["wallets:read"], idempotency_key: crypto.randomUUID() },
});
```

Call permission and signing requests from a click handler: an extension may refuse them without user activation (BROWSER §5.2). Rejections are `Error` objects with `name === "PwcError"` and a Core `code`.

## 6. Amounts and fees

- Amounts are decimal strings of satoshis (Core §3.2). Parse them into exact integers, such as `BigInt` in JavaScript or `int` in Python, and never into floating point.
- A `FeeRate` is in sat/kvB: `"1250"` is 1.25 sat/vB. To convert a sat/vB rate entered by a User, multiply by 1,000 and round to an integer before sending.
- The fee for a virtual size is `ceil(rate × vsize / 1000)` (Core §3.4). The test vectors FEE-01 to FEE-07 check this.
- The effective value of an account is `confirmed_sats + unconfirmed_delta_sats`. Show `spendable_sats` as the amount available to send, and treat `pending_external_sats` as not yet reliable (Core §4.3).

## 7. Transaction flows

**Single signer:**

1. `psbt.create`: poll the Operation and show `fee_sats` from its result.
2. `psbt.sign`: poll the Operation; the User approves in the wallet.
3. `psbt.finalize`: if `complete` is true, take `raw_transaction`.
4. `transaction.broadcast`, then watch `transaction.get` for confirmations.

**Multisignature:** have each cosigner's Provider run `psbt.sign` on the same PSBT, merge the results with `psbt.combine`, then finalize and broadcast. Only version 0 PSBTs are accepted (Core §4.7).

**Reservations:** if `psbt.create` returns a `reservation_id` and you abandon the transaction, call `psbt.releaseReservation` (Core §8.3).

Never broadcast a transaction you did not inspect, and never treat broadcast acceptance as confirmation.

## 8. Handling errors

| Code | What the Client should do |
|---|---|
| `UNAUTHENTICATED` | Authenticate again; if that fails, pair again |
| `PERMISSION_REQUIRED` | Request the missing scope |
| `PERMISSION_REVOKED` | Tell the User access was withdrawn; request again only if the User asks |
| `WALLET_NOT_FOUND`, `ACCOUNT_NOT_FOUND` | Refresh `wallet.list` and `account.list` |
| `WALLET_LOCKED` | Ask the User to unlock the wallet, then retry |
| `USER_REJECTED` | Stop. Do not prompt again immediately |
| `RATE_LIMITED` | Back off, from 1 second doubling to 30 seconds |
| `PROVIDER_UNAVAILABLE` | Ask the User to open the wallet; retry with the same key |
| `SYNC_INCOMPLETE`, `STATE_UNAVAILABLE` | Retry later; show existing data as stale |
| `CURSOR_EXPIRED` | Restart the listing from the first page |
| `IDEMPOTENCY_CONFLICT` | Fix the Client: a key was reused for a different action |
| `INSUFFICIENT_FUNDS`, `POLICY_VIOLATION`, `GRANT_CONSTRAINT_VIOLATED` | Show the reason; let the User change the request |
| `PSBT_VALIDATION_FAILED`, `PSBT_CONFLICT`, `UNSUPPORTED_PSBT_VERSION` | Fix the PSBT; do not resubmit it unchanged |
| `TRANSACTION_REJECTED` | Show `details`; do not rebroadcast unchanged |
| `INTERNAL_ERROR` | Retry later; report if it persists |

Branch on `code`, never on `message` (Core §10). Respect `retryable` when present.

## 9. Privacy

- Request the fewest scopes. `utxos:read`, `transactions:read` and `psbt:create` reveal the most (PWC-UX-001 §3).
- Do not store wallet data you do not need, and do not send it to your servers without telling the User.
- Remember that a created PSBT reveals the coins it spends (Core §4.7).

## 10. Testing

- **Mock Provider:** `python3 mock_local_provider.py --dir <dir>` from the test kit implements the LOCAL handshake and a few Core methods with fixed data.
- **Networks:** develop against `regtest` or `signet` (Core §3.2) before touching `mainnet`.
- **Vectors:** use `vectors/*.json` to test amount parsing, fees, balance interpretation and PSBT handling.
- **Schemas:** validate what you send and receive with the files in `pwc-schema/` (PWC-CONFORMANCE-001 §5).

## 11. Checklist

- [ ] Methods and limits taken from `pwc.getCapabilities`, not assumed
- [ ] Only the needed scopes requested
- [ ] One idempotency key per intended action, reused on retry
- [ ] Amounts parsed as exact integers
- [ ] Every error branch keyed on `code`
- [ ] Credentials stored in the platform's secure storage (LOCAL, REMOTE)
- [ ] SAS confirmation required before using a REMOTE or MOBILE pairing
- [ ] Revocation, locking and unavailability handled without crashing

## 12. References

- PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001, -BROWSER-001, -REMOTE-001 and -MOBILE-001 v0.1.0
- PWC-UX-001 v0.1.0; PWC-CONFORMANCE-001 v0.1.0

---

**End of PWC-CLIENT-GUIDE-001 v0.1.0.**
