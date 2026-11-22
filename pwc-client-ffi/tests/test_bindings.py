"""TASK-FFI-002: the generated Python module against the Rust Provider.

Usage: python3 test_bindings.py <directory of pwc_client.py> <endpoint directory> <scratch directory>
Run by tests/python.rs, which starts a Provider whose prompts are approved after 200 ms.
"""
import asyncio
import os
import sys
from datetime import timedelta

sys.path.insert(0, sys.argv[1])
import pwc_client as pwc  # noqa: E402

ACCOUNT = "a-regtest-wpkh"
WAIT = timedelta(seconds=10)
SCOPES = ["wallets:read", "balances:read", "addresses:read", "utxos:read", "transactions:read",
          "psbt:create", "psbt:sign", "transactions:broadcast", "events:read"]


class MemoryStore(pwc.CredentialStore):
    """A credential store written in Python; a real application would use the keyring."""

    def __init__(self):
        self.credentials = {}
        self.fail = False

    def load(self, provider_id):
        if self.fail:
            raise RuntimeError("keyring is locked")
        return self.credentials.get(provider_id)

    def save(self, credential):
        self.credentials[credential.provider_id] = credential

    def delete(self, provider_id):
        self.credentials.pop(provider_id, None)


async def raises(variant, awaitable):
    try:
        await awaitable
    except variant as e:
        return e
    raise AssertionError(f"expected {variant.__name__}")


async def main():
    assert os.path.isabs(pwc.endpoint_directory())
    endpoints = pwc.discover(sys.argv[2])
    assert [e.provider_id for e in endpoints] == ["pwc-provider"], endpoints
    assert endpoints[0].path == os.path.join(sys.argv[2], "pwc-provider.sock")
    endpoint = endpoints[0]

    store = MemoryStore()
    await raises(pwc.Error.NotPaired, pwc.Client.connect(endpoint, store))
    store.fail = True
    e = await raises(pwc.Error.Io, pwc.Client.connect(endpoint, store))
    assert "keyring is locked" in e[0], e
    store.fail = False

    code = pwc.pairing_code()
    assert len(code) == 6 and code.isdigit()
    paired = await pwc.Client.pair(endpoint, store, pwc.PairParams(
        client_name="Python Test", client_description=None, pairing_code=code))
    credential = store.credentials["pwc-provider"]
    assert credential.client_id == paired.client_id() and len(credential.client_secret) == 64
    await paired.close()

    client = await pwc.Client.connect(endpoint, store)
    assert client.client_id() == paired.client_id()
    assert client.hello().provider_id == "pwc-provider"

    caps = await client.get_capabilities()
    assert caps.pwc_version == "0.2.1" and len(caps.methods) == 24
    assert pwc.ScriptType.P2WPKH in caps.script_types
    assert isinstance(caps.limits.high_fee_rate, int)

    e = await raises(pwc.Error.Pwc, client.list_wallets())
    assert e[0].code == pwc.ErrorCode.PERMISSION_REQUIRED(), e

    op = await client.request_permissions(pwc.RequestPermissionsParams(
        scopes=SCOPES, network="regtest", wallet_ids=None, account_ids=None, expires_at=None,
        constraints=None, idempotency_key=pwc.idempotency_key()))
    assert op.state == pwc.OperationState.AWAITING_APPROVAL and not op.is_terminal()
    try:
        op.permissions_result()
        raise AssertionError("a pending Operation has no result")
    except pwc.Error.OperationPending:
        pass
    await raises(pwc.Error.Timeout, client.wait_operation(op.operation_id, timedelta(milliseconds=50)))
    events = client.subscribe_events()
    op = await client.wait_operation(op.operation_id, WAIT)
    grants = op.permissions_result()
    assert op.is_terminal() and len(grants) == 1 and grants[0].status == pwc.GrantStatus.ACTIVE
    assert isinstance(op.result, dict) and op.result["grants"][0]["grant_id"] == grants[0].grant_id

    wallets = await client.list_wallets()
    assert "w-regtest-main" in [w.wallet_id for w in wallets]
    accounts = await client.list_accounts("w-regtest-main")
    assert any(a.account_id == ACCOUNT and a.can_sign for a in accounts)
    e = await raises(pwc.Error.Pwc, client.list_accounts("w-mainnet-main"))
    assert e[0].code == pwc.ErrorCode.WALLET_NOT_FOUND()

    balance = await client.get_balance(ACCOUNT)
    assert balance.confirmed_sats == 1_500_000 and isinstance(balance.unconfirmed_delta_sats, int)
    utxos = await client.list_all_utxos(ACCOUNT)
    assert sorted(u.value_sats for u in utxos) == [500_000, 1_000_000]
    page = await client.list_utxos(ACCOUNT, None, 1)
    assert len(page.utxos) == 1 and page.next_cursor is not None

    to = (await client.get_receive_address("a-regtest-pkh")).address
    op = await client.create_psbt(pwc.CreatePsbtParams(
        account_id=ACCOUNT, outputs=[pwc.Payment(address=to, amount_sats=300_000)],
        fee_policy=pwc.FeePolicy.RATE(sat_per_kvb=2000), selected_utxos=None, replaceable=None,
        idempotency_key=pwc.idempotency_key()))
    created = (await client.wait_operation(op.operation_id, WAIT)).create_psbt_result()
    assert created.fee_sats == -(-2000 * created.vsize // 1000)

    op = await client.sign_psbt(pwc.SignPsbtParams(
        account_id=ACCOUNT, psbt=created.psbt, input_indexes=None, idempotency_key=pwc.idempotency_key()))
    signed = (await client.wait_operation(op.operation_id, WAIT)).sign_psbt_result()
    assert signed.signing_status == pwc.SigningStatus.COMPLETE

    seen = []
    while not (seen and seen[-1].operation_id == op.operation_id and seen[-1].payload == {"state": "completed"}):
        seen.append(await asyncio.wait_for(events.next(), 10))
    assert all(e.event_type == "operation.stateChanged" for e in seen), seen

    done = await client.finalize_psbt(signed.psbt)
    assert done.complete
    op = await client.broadcast_transaction(ACCOUNT, done.raw_transaction, pwc.idempotency_key())
    txid = (await client.wait_operation(op.operation_id, WAIT)).broadcast_result()
    assert txid == done.txid
    tx = await client.get_transaction(ACCOUNT, txid)
    assert tx.status == pwc.TransactionStatus.UNCONFIRMED and tx.net_change_sats == -(300_000 + created.fee_sats)

    raw = await client.request("operation.get", {"operation_id": op.operation_id})
    assert raw["state"] == "completed" and raw["result"] == {"txid": txid}
    e = await raises(pwc.Error.Pwc, client.request("vendor.com.example.none", {}))
    assert e[0].code == pwc.ErrorCode.UNSUPPORTED_METHOD()
    e = await raises(pwc.Error.Pwc, client.wait_operation("op-none", WAIT))
    assert e[0].code == pwc.ErrorCode.OPERATION_NOT_FOUND()

    files = pwc.file_credential_store(os.path.join(sys.argv[3], "creds"))
    assert files.load("pwc-provider") is None
    files.save(credential)
    assert files.load("pwc-provider") == credential
    assert os.stat(os.path.join(sys.argv[3], "creds", "pwc-provider.json")).st_mode & 0o777 == 0o600
    other = await pwc.Client.connect(endpoint, files)
    assert other.session_id() != client.session_id()
    await other.close()
    await raises(pwc.Error.ConnectionClosed, other.get_session_info())

    await client.unpair()
    assert store.credentials == {}
    e = await raises(pwc.Error.Pwc, pwc.Client.connect(endpoint, files))
    assert e[0].code == pwc.ErrorCode.UNAUTHENTICATED()
    print("python bindings: all checks passed")


asyncio.run(main())
