# pwc-client-ffi

Bindings of [`pwc-client`](../pwc-client/) for other languages, generated with [UniFFI](https://mozilla.github.io/uniffi-rs/). This crate builds the C dynamic library and holds the generator program; the interface itself is annotated in `pwc-client` behind its feature `uniffi`. Python is the first language. macOS and Linux.

## Python

Build the library and generate the module `pwc_client`:

```bash
cargo build --release --lib
```

```bash
cargo run --release --bin uniffi-bindgen -- generate --language python --library target/release/libpwc_client_ffi.so --out-dir python
```

```bash
cp target/release/libpwc_client_ffi.so python/
```

On macOS the library is `libpwc_client_ffi.dylib`. Put the directory `python/` on the Python path; `pwc_client.py` loads the library from its own directory. Python 3.9 or later; no Python package is needed.

```python
import asyncio
from datetime import timedelta
import pwc_client as pwc

async def main():
    endpoint = pwc.discover(pwc.endpoint_directory())[0]
    store = pwc.file_credential_store("/home/me/.config/myapp/pwc")   # or a subclass of pwc.CredentialStore
    try:
        client = await pwc.Client.connect(endpoint, store)
    except pwc.Error.NotPaired:
        code = pwc.pairing_code()
        print("Compare this code with the wallet:", code)
        client = await pwc.Client.pair(endpoint, store, pwc.PairParams(
            client_name="My App", client_description=None, pairing_code=code))

    op = await client.request_permissions(pwc.RequestPermissionsParams(
        scopes=["wallets:read", "balances:read"], network=None, wallet_ids=None, account_ids=None,
        expires_at=None, constraints=None, idempotency_key=pwc.idempotency_key()))
    op = await client.wait_operation(op.operation_id, timedelta(minutes=5))
    try:
        grants = op.permissions_result()
    except pwc.Error.Pwc as e:
        print("not granted:", e[0].code)       # for example ErrorCode.USER_REJECTED()
        return
    for wallet in await client.list_wallets():
        print(wallet.display_name, wallet.state)

asyncio.run(main())
```

The interface is that of the Rust library; the differences are listed in [`docs/api-contract.md`](../docs/api-contract.md). A complete program is [`tests/test_bindings.py`](tests/test_bindings.py), which `cargo test` runs against the Rust Provider.
