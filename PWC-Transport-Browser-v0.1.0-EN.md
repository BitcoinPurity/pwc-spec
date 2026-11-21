# Purity Wallet Connect (PWC)
## Transport Profile BROWSER: Web Pages Through a Browser Extension

**Specification:** PWC-TRANSPORT-BROWSER-001  
**Version:** 0.1.0 (review draft; not a deployed standard)  
**Date:** 2026-11-21  
**Language:** English (normative draft)  
**License:** CC BY 4.0  
**Requires:** PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001 v0.1.0 for bridge mode (§7)  
**Status:** Proposed specification, subject to owner review and security audit

> **Scope rule:** This profile defines how a web page, acting as a Client, reaches a Provider through a browser extension: how the page discovers Providers, the JavaScript interface it calls, how the web origin becomes the authenticated principal, and how an extension that is only a bridge reaches a desktop Provider. It changes no Core semantics.

## 0. Decisions, status and interpretation

### 0.1 Decisions adopted in v0.1.0

- The Client principal is the **web origin** of the calling page, as determined by the browser. Nothing a page supplies about itself is trusted.
- Pages discover Providers through two DOM events, `pwc:requestProvider` and `pwc:announceProvider`, so that several wallets can coexist without competing for a global variable.
- The page-facing object exposes one method, `request({ method, params })`, plus listeners for events.
- Only top-level documents in secure contexts are served.
- Two deployment modes share the same page interface: the extension is itself the Provider (*extension mode*), or the extension relays to a desktop Provider over LOCAL delegated principals (*bridge mode*).

The present version remains a **review draft**. Open questions are listed in Appendix A.

### 0.2 Normative vocabulary

Requirement keywords have the meanings given in PWC-CORE-001 §0.4. Core terms and semantic types are used as defined there. "Core §n" points into PWC-CORE-001 v0.2.1, and "LOCAL §n" into PWC-TRANSPORT-LOCAL-001 v0.1.0. *Origin* and *secure context* are used as defined by the HTML Standard.

## 1. Scope and deployment modes

### 1.1 Roles

| Component | Role |
|---|---|
| Web page | The Client. Its principal is its origin |
| Page interface | The JavaScript object a page receives through discovery (§4). It holds no secret and no authority |
| Extension | Determines the origin of each request from browser APIs, shows prompts in its own interface, and either acts as the Provider or relays to one |
| Bridge host | In bridge mode only: a native messaging host shipped with the extension, which is a LOCAL delegator Client of the desktop Provider |
| Desktop Provider | In bridge mode only: a LOCAL Provider that implements LOCAL §5.8 |

### 1.2 Modes

- **Extension mode.** The extension holds the wallet and implements Core directly. `Capabilities.provider.platform` is `extension`.
- **Bridge mode.** The extension forwards each request to its bridge host, which forwards it as a LOCAL delegated request (LOCAL §5.8) to the desktop Provider. Capabilities, results and errors are the desktop Provider's own.

Both modes present the same page interface. A page cannot tell them apart except through `Capabilities.provider`.

### 1.3 Not covered

Mobile browsers without extension support, in-app web views, and web pages that reach a wallet on another device are not covered here; they use PWC-TRANSPORT-MOBILE or PWC-TRANSPORT-REMOTE.

## 2. Threat model

| Attacker | Protected | Mechanism |
|---|---|---|
| Malicious web page | Yes | Its origin is its principal; it obtains nothing without a grant to that origin (§3) |
| Look-alike origin used for phishing | Partly | Prompts show the origin prominently (§6). The User still has to read it |
| Cross-origin frame embedded in a legitimate page | Yes | Frames are not served (§3.2) |
| Page forging its own identity (name, icon, URL) | Yes | Only the browser-determined origin counts (§3.1) |
| Page overlaying or clickjacking the prompt | Yes | Prompts appear only in extension interface (§6) |
| Page reading another page's responses | Yes | Responses are delivered only to the requesting document (§5.3) |
| Script injected into a legitimate origin (XSS, a compromised third-party script) | **No** | Such code runs as that origin and holds its grants. Per-action approval (Core §9.2) remains |
| Another extension with access to the page | **No** | It can act as the page. Out of scope |
| Malicious bridge host or desktop process of the same user | **No** | As LOCAL §2 |

## 3. The Client principal

### 3.1 Origin as principal

The extension MUST determine the origin of every request from browser APIs, for example the `origin` of the `runtime.MessageSender` of the message that carried it. It MUST NOT use any value supplied by the page, such as `location`, a claimed name or a URL inside the request.

The Provider assigns one `client_id` per origin and browser profile. In extension mode the extension does this itself; in bridge mode the desktop Provider does it through `local.openDelegate` (§7). Grants, Operations, idempotency records and rate limits belong to that `client_id`.

### 3.2 Which documents are served

The extension MUST announce a Provider (§4.1) only into documents that meet all of these conditions:

- the document is a top-level document, not a frame;
- it is a secure context;
- its origin is an `https` origin, or an `http` origin whose host is `localhost` or a loopback address.

Opaque origins, `file:` URLs and extension pages of other extensions are never served. If a request nevertheless arrives from a document that does not meet these conditions, the extension MUST fail it with `UNAUTHENTICATED`.

### 3.3 Private browsing

Documents in a private-browsing context are a separate principal from the same origin in normal browsing. Their grants and Operations MUST NOT outlive the private-browsing session. In bridge mode this is achieved with `private: true` (LOCAL §5.8).

### 3.4 No pairing step

The browser vouches for the origin, so there is no pairing and no secret. The first User decision an origin meets is its first `session.requestPermissions` prompt. Before any grant, the page can still call the methods that need no scope (Core §6).

## 4. Page interface

### 4.1 Discovery

```typescript
type PwcProviderInfo = {
  uuid: string;            // random UUID, new for each document
  provider_id: string;     // [a-z0-9-]{1,32}, stable, e.g. "puritywallet"
  name: Text;              // display name
  icon: string;            // data: URI of a square PNG, WebP or SVG image
  protocol: "pwc-browser/1";
};
type PwcProviderDetail = { info: PwcProviderInfo; provider: PwcProvider };
```

The protocol works as follows:

- A page that wants Providers dispatches `new Event("pwc:requestProvider")` on `window`.
- Each extension, on document start and whenever it observes `pwc:requestProvider`, dispatches on `window` a `CustomEvent("pwc:announceProvider", { detail })`, where `detail` is a `PwcProviderDetail` frozen with `Object.freeze`.
- A page that finds several Providers SHOULD let the User choose one.
- A page MUST render `icon` only through an `img` element or equivalent, never by inserting it as markup, because an SVG can contain script.
- `name`, `icon` and `provider_id` are claims of the extension. A page MUST NOT use them as proof of which software it is talking to.

Extensions MUST NOT define a global variable for the Provider.

### 4.2 `PwcProvider`

```typescript
interface PwcProvider {
  request(args: { method: string; params?: object }): Promise<object>;
  addEventListener(type: "pwcevent", listener: (e: CustomEvent<PwcEvent>) => void): void;
  removeEventListener(type: "pwcevent", listener: (e: CustomEvent<PwcEvent>) => void): void;
}
```

The following rules apply to `request`:

- `method` is a Core method name. `params` defaults to `{}`.
- The extension MUST convert `params` to JSON before forwarding it. A value that cannot be represented in JSON fails the request with `INVALID_ARGUMENT`. Functions, prototypes and other non-JSON content are never passed on (Core §3.1).
- The promise resolves with the Core result object, which the page receives as a fresh JSON-derived object.
- The promise rejects with an `Error` whose `name` is `"PwcError"` and which carries the Core `PwcError` members `code`, `message` and, where present, `details` and `retryable`.
- The extension evaluates every request with the order of checks of Core §8.

When the extension cannot reach the Provider, the promise rejects with `PROVIDER_UNAVAILABLE`. In bridge mode this happens when the bridge host or the desktop Provider is not running.

### 4.3 Events

A page that holds `events:read` receives Core events as `pwcevent` events on its `PwcProvider`, with `detail` set to the `PwcEvent`. The disclosure rules of Core §8.6 apply to the page's `client_id`. Events are not replayed after a reload; the page re-queries state.

## 5. Sessions and extension internals

### 5.1 Session

| Core concept | BROWSER binding |
|---|---|
| Authenticated principal | The origin, as determined by the browser |
| `client_id` | One per origin and browser profile; a separate one for private browsing |
| Session | One document's use of one Provider |
| `session_id` | Assigned at the document's first request; ends on navigation, reload, closing the tab, or discarding the document |

Grants and Operations belong to the `client_id`, so a reloaded page can recover an Operation with `operation.get` or by retrying with the same `idempotency_key` (Core §9.3). When a session ends, the extension MUST stop delivering its events and MUST NOT deliver responses of requests still outstanding.

### 5.2 Prompt rate limit

The rate limit of Core §12, item 13 applies per origin. An extension SHOULD also require that `session.requestPermissions`, `psbt.sign` and `message.sign` be called while the page has transient user activation, and MAY refuse them otherwise with `POLICY_VIOLATION`.

### 5.3 Message path inside the browser

The path from page to extension is not standardized, but every implementation MUST meet these requirements:

- The page interface runs in the page's own JavaScript environment and holds no secret, credential or grant.
- Any code of the extension that receives messages from the page interface MUST accept only messages from its own document, and MUST deliver each response only to the document that sent the request.
- The component that decides authorization MUST take the origin from browser APIs (§3.1), MUST verify that the sender is a top-level document (for example, `frameId` 0), and MUST NOT rely on any field of the message for either.
- Prompts are never rendered inside the page (§6).

*Informative:* in a Manifest V3 extension, the page interface is typically injected into the page's main world, a content script in the isolated world relays messages, and the service worker reads `MessageSender.origin` and `MessageSender.frameId`.

## 6. Prompts

- Every prompt MUST be shown in extension interface, such as a popup or a dedicated window, and never as content of the page.
- Every prompt MUST show the requesting origin prominently, with its registrable domain emphasized, and MUST mark any page-supplied name or icon as claimed by the website.
- A prompt for a private-browsing principal SHOULD say so.
- In bridge mode the desktop Provider also shows its own prompt (LOCAL §5.8). The extension MUST NOT present its forwarding of a request as an approval of it.

## 7. Bridge mode

### 7.1 Components

The extension ships a native messaging host, the *bridge host*. The browser starts it on request and restricts it to the extension through the host manifest's `allowed_origins` (Chrome) or `allowed_extensions` (Firefox). The bridge host is a LOCAL Client paired with the desktop Provider as a delegator (LOCAL §5.8). It stores the LOCAL credential under LOCAL §7.

### 7.2 Request flow

1. When a document makes its first request, the extension asks the bridge host to open a delegated session with `local.openDelegate`. The principal is `{ kind: "web_origin", origin: <origin from §3.1>, private: <whether the document is in private browsing> }`.
2. Each request of the document is forwarded as `local.delegated` with that session's `delegate_id`.
3. `local.delegatedEvent` notifications are delivered to the document of that `delegate_id` as `pwcevent` events (§4.3).
4. When the document's session ends (§5.1), the extension closes the delegated session with `local.closeDelegate`.

The extension, not the bridge host, determines the origin. The bridge host MUST accept messages only from its own extension, and MUST NOT open a delegated session for an origin that the extension did not report.

### 7.3 Message size

Chrome limits a single message from a native messaging host to the extension to 1 MB, while messages to the host may be up to 64 MiB. The extension and its bridge host MUST be able to carry any frame up to the desktop Provider's `max_frame_bytes`, for example by splitting large frames. How they do so is private to the extension, because both parts ship together.

### 7.4 Availability

If the bridge host cannot be started, is not paired, or cannot reach the desktop Provider, the extension fails requests with `PROVIDER_UNAVAILABLE`. It SHOULD then tell the User what is missing.

## 8. Mapping of Core obligations

| Core requirement | Where this profile meets it |
|---|---|
| Authenticate the Client principal and bind it to the grant (Core §2) | §3.1, §5.1, §7.2 |
| Discovery discloses no wallet data (Core §7, step 1) | §4.1: announcements carry only Provider identity |
| Client identity is not taken from display metadata (Core §2) | §3.1, §6 |
| Outer request/response envelope (Core §3.1) | §4.2 |
| Error mapping (Core §10) | §4.2 |
| Event delivery (Core §8.6) | §4.3, §7.2 |
| Authenticated, confidential, replay-resistant binding (Core §12, item 9) | Browser-enforced origin and document isolation, §3 and §5.3; LOCAL for the bridge hop |
| Cross-origin and callback attacks (Core §12, item 9) | §3.2, §5.3 |
| Prompt flooding (Core §12, item 13) | §5.2 |

## 9. Security considerations

1. **The origin is the identity.** Every guarantee in this profile rests on the browser's origin determination. Implementations MUST NOT add any fallback that accepts a page-supplied identity.
2. **Same-origin code.** Any script running in an origin holds that origin's grants. Pages SHOULD use a Content Security Policy and avoid untrusted third-party scripts before requesting wallet access.
3. **Phishing.** Look-alike domains defeat a User who does not read the origin. Providers SHOULD warn when an origin resembles one the User has already granted.
4. **Bridge trust.** In bridge mode the desktop Provider trusts the origins the bridge reports. Delegator pairing is a high-trust decision (LOCAL §5.8).
5. **No secrets in the page.** The page interface carries no secret, so a page that tampers with its own copy gains nothing.

## 10. Conformance tests

| ID | Test | Required outcome |
|---|---|---|
| B-01 | `pwc:requestProvider` from a top-level `https` page | One `pwc:announceProvider` per extension, with a frozen `detail` |
| B-02 | Page in a cross-origin frame | No announcement; a forced request fails with `UNAUTHENTICATED` |
| B-03 | Page with an opaque origin or a `file:` URL | No announcement |
| B-04 | `http` page on a non-loopback host | No announcement |
| B-05 | Request whose `params` claim another origin | Evaluated as the real origin only |
| B-06 | Grant made to origin A, request from origin B | `PERMISSION_REQUIRED` for B |
| B-07 | Same origin in normal and private browsing | Separate grants; private grants gone after the private session ends |
| B-08 | `params` containing a function or a cyclic object | `INVALID_ARGUMENT` |
| B-09 | Core error | Promise rejects with `name` `"PwcError"` and the Core `code` |
| B-10 | Prompt | Shown outside the page, origin prominent, page-supplied name marked as claimed |
| B-11 | Reload while `psbt.sign` is pending, then retry with the same key | Same `operation_id` |
| B-12 | Events with and without `events:read` | `pwcevent` delivered only with the scope |
| B-13 | Response to a request from another tab of the same origin | Delivered only to the requesting document |
| B-14 | Bridge mode with the bridge host absent or the desktop Provider stopped | `PROVIDER_UNAVAILABLE` |
| B-15 | Bridge mode: document session ends | `local.closeDelegate` sent; no further events delivered |
| B-16 | Bridge mode: response larger than 1 MB | Delivered intact |
| B-17 | Approval-creating request without user activation, where the extension requires it | `POLICY_VIOLATION`, no prompt |
| B-18 | Global variables | No Provider global defined by the extension |

## 11. Informative example

```javascript
const providers = new Map();
window.addEventListener("pwc:announceProvider", (e) => {
  providers.set(e.detail.info.uuid, e.detail);
});
window.dispatchEvent(new Event("pwc:requestProvider"));

// After the User picks one:
const { provider } = providers.values().next().value;
try {
  const op = await provider.request({
    method: "session.requestPermissions",
    params: { scopes: ["wallets:read", "balances:read"], idempotency_key: crypto.randomUUID() },
  });
  console.log(op.state);                      // "awaiting_approval"
} catch (err) {
  if (err.name === "PwcError" && err.code === "PROVIDER_UNAVAILABLE") {
    // ask the User to start the wallet
  }
}
provider.addEventListener("pwcevent", (e) => console.log(e.detail.event_type));
```

The idempotency key above is a UUID, which matches the `Id` syntax of Core §3.2.

## 12. References

- PWC-CORE-001 v0.2.1; PWC-TRANSPORT-LOCAL-001 v0.1.0
- HTML Standard, origins and secure contexts: https://html.spec.whatwg.org/multipage/browsers.html#origin
- RFC 6454, The Web Origin Concept: https://www.rfc-editor.org/rfc/rfc6454
- RFC 2397, the `data:` URL scheme: https://www.rfc-editor.org/rfc/rfc2397
- Chrome native messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- MDN, `runtime.MessageSender`: https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/runtime/MessageSender
- EIP-6963, Multi Injected Provider Discovery (informative; the discovery events of §4.1 follow its pattern): https://eips.ethereum.org/EIPS/eip-6963

## Appendix A. Open questions

| ID | Subject | Current draft policy |
|---|---|---|
| BA-01 | Cross-origin frames | Not served. A later version may serve them with a prompt naming both the frame and the top-level origin |
| BA-02 | Transient user activation | SHOULD, not MUST (§5.2) |
| BA-03 | Mobile browsers with extension support | Not separately addressed; the same rules apply where the browser supports them |
| BA-04 | Detecting look-alike origins | SHOULD warn (§9); no algorithm defined |

---

**End of PWC-TRANSPORT-BROWSER-001 v0.1.0 English review draft.**
