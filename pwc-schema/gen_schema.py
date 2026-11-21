"""Generate the PWC JSON Schema files (JSON Schema 2020-12).

Output, written next to this script:
  pwc-core-0.2.1.schema.json          permissive: unknown members allowed (Core §3.1)
  pwc-core-0.2.1.strict.schema.json   strict: no unknown members (conformance testing, e.g. C-28)
  pwc-transport-local-0.1.0.schema.json
  pwc-transport-browser-0.1.0.schema.json
  pwc-transport-remote-0.1.0.schema.json
"""
import copy
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
CORE_ID = "https://purity-wallet-connect.invalid/schema/pwc-core-0.2.1.schema.json"
CORE_STRICT_ID = "https://purity-wallet-connect.invalid/schema/pwc-core-0.2.1.strict.schema.json"
LOCAL_ID = "https://purity-wallet-connect.invalid/schema/pwc-transport-local-0.1.0.schema.json"
BROWSER_ID = "https://purity-wallet-connect.invalid/schema/pwc-transport-browser-0.1.0.schema.json"
REMOTE_ID = "https://purity-wallet-connect.invalid/schema/pwc-transport-remote-0.1.0.schema.json"

MAX_SATS_16 = r"1[0-9]{15}|20[0-9]{14}|2100000000000000"
SAFE_INT = 9007199254740991


def ref(name):
    return {"$ref": f"#/$defs/{name}"}


def arr(items, **kw):
    return {"type": "array", "items": items, **kw}


def obj(props, required=(), **kw):
    o = {"type": "object", "properties": props, "required": sorted(required)}
    o.update(kw)
    return o


def merge(*objs):
    props, req, extra = {}, set(), {}
    for o in objs:
        props.update(o["properties"])
        req.update(o["required"])
        for k, v in o.items():
            if k not in ("type", "properties", "required"):
                extra.setdefault(k, []).append(v)
    out = obj(props, req)
    if "allOf" in extra:
        out["allOf"] = [x for lst in extra["allOf"] for x in lst]
    return out


def enum(*values):
    return {"type": "string", "enum": list(values)}


CORE_ERROR_CODES = [
    "INVALID_ARGUMENT", "PAYLOAD_TOO_LARGE", "UNSUPPORTED_METHOD", "UNSUPPORTED_OPERATION",
    "UNSUPPORTED_PSBT_VERSION", "UNSUPPORTED_SCRIPT", "UNAUTHENTICATED", "PERMISSION_REQUIRED",
    "PERMISSION_REVOKED", "GRANT_NOT_FOUND", "GRANT_CONSTRAINT_VIOLATED", "USER_REJECTED",
    "WALLET_LOCKED", "WALLET_NOT_FOUND", "ACCOUNT_NOT_FOUND", "NETWORK_MISMATCH",
    "NETWORK_UNAVAILABLE", "SYNC_INCOMPLETE", "STATE_UNAVAILABLE", "SIGNER_UNAVAILABLE",
    "INSUFFICIENT_FUNDS", "POLICY_VIOLATION", "PSBT_VALIDATION_FAILED", "PSBT_CONFLICT",
    "TRANSACTION_REJECTED", "TRANSACTION_NOT_FOUND", "RESERVATION_NOT_FOUND",
    "OPERATION_NOT_FOUND", "OPERATION_NOT_CANCELABLE", "OPERATION_EXPIRED", "OPERATION_CANCELED",
    "IDEMPOTENCY_CONFLICT", "CURSOR_EXPIRED", "RATE_LIMITED", "PROVIDER_UNAVAILABLE", "INTERNAL_ERROR",
]
CORE_SCOPES = [
    "wallets:read", "balances:read", "addresses:read", "addresses:create", "transactions:read",
    "utxos:read", "psbt:create", "psbt:sign", "transactions:broadcast", "messages:sign", "events:read",
]
CORE_FEATURES = ["coin_control", "multisig", "hardware_signing", "watch_only", "event_push"]
CORE_EVENTS = [
    "wallet.locked", "wallet.unlocked", "wallet.syncChanged", "account.balanceChanged",
    "account.transactionChanged", "operation.stateChanged", "grant.revoked", "grant.expired",
    "session.expired",
]
VENDOR = r"vendor(\.[a-z0-9-]+)+\.[A-Za-z][A-Za-z0-9_-]*"

OP_STATES = ["queued", "awaiting_approval", "awaiting_unlock", "awaiting_signer", "processing",
             "completed", "rejected", "expired", "canceled", "failed"]
TX_STATUS = ["unconfirmed", "confirmed", "conflicted", "replaced", "dropped", "unknown"]


def primitives():
    return {
        "Id": {"type": "string", "pattern": r"^[A-Za-z0-9._~:-]{1,128}$"},
        "Sats": {"type": "string", "pattern": rf"^(0|[1-9][0-9]{{0,14}}|{MAX_SATS_16})$"},
        "PositiveSats": {"type": "string", "pattern": rf"^([1-9][0-9]{{0,14}}|{MAX_SATS_16})$"},
        "SignedSats": {"type": "string", "pattern": rf"^(0|-?([1-9][0-9]{{0,14}}|{MAX_SATS_16}))$"},
        "FeeRate": {"type": "string", "pattern": r"^([1-9][0-9]{0,8}|1000000000)$"},
        "UInt32": {"type": "integer", "minimum": 0, "maximum": 4294967295},
        "NonNegativeInteger": {"type": "integer", "minimum": 0, "maximum": SAFE_INT},
        "PositiveInteger": {"type": "integer", "minimum": 1, "maximum": SAFE_INT},
        "BlockHeight": {"type": "integer", "minimum": 0, "maximum": 4294967295},
        "PsbtVersion": {"const": 0},
        "Timestamp": {"type": "string",
                      "pattern": r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]+)?Z$",
                      "format": "date-time"},
        "Hash256": {"type": "string", "pattern": r"^[0-9a-f]{64}$"},
        "Hex": {"type": "string", "pattern": r"^([0-9a-f]{2})+$"},
        "Base64": {"type": "string",
                   "pattern": r"^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{4}|[A-Za-z0-9+/][AQgw]==|[A-Za-z0-9+/]{2}[AEIMQUYcgkosw048]=)$"},
        "Address": {"type": "string", "minLength": 1},
        "Network": {"type": "string", "pattern": r"^[a-z0-9-]{1,32}$"},
        "Cursor": {"type": "string", "minLength": 1},
        "Text": {"type": "string", "minLength": 1, "maxLength": 256,
                 "pattern": "^[^\\u0000-\\u001f\\u007f-\\u009f]*$"},
        "MethodName": {"type": "string", "pattern": rf"^([a-z]+\.[a-z][A-Za-z]*|{VENDOR})$"},
        "Scope": {"anyOf": [enum(*CORE_SCOPES), {"type": "string", "pattern": rf"^{VENDOR}$"}]},
        "Feature": {"anyOf": [enum(*CORE_FEATURES), {"type": "string", "pattern": rf"^{VENDOR}$"}]},
        "ErrorCode": {"anyOf": [enum(*CORE_ERROR_CODES), {"type": "string", "pattern": rf"^{VENDOR}$"}]},
        "EventType": {"anyOf": [enum(*CORE_EVENTS), {"type": "string", "pattern": rf"^{VENDOR}$"}]},
        "ProfileName": {"type": "string", "pattern": r"^[a-z0-9-]+-v[0-9]+$"},
        "SemVer": {"type": "string",
                   "pattern": r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$"},
    }


def composites():
    d = {}
    d["OutPoint"] = obj({"txid": ref("Hash256"), "vout": ref("UInt32")}, ["txid", "vout"])
    d["ScriptType"] = enum("p2pkh", "p2sh", "p2sh-p2wpkh", "p2sh-p2wsh", "p2wpkh", "p2wsh", "p2tr", "other")
    d["MessageFormat"] = enum("bip322-simple", "bip322-full", "legacy")
    d["SyncStatus"] = obj({"state": enum("synced", "syncing", "offline", "unknown"),
                           "tip_height": ref("BlockHeight")}, ["state"])
    d["FeePolicy"] = {"oneOf": [
        obj({"type": {"const": "rate"}, "sat_per_kvb": ref("FeeRate")}, ["type", "sat_per_kvb"]),
        obj({"type": {"const": "wallet_estimate"}, "target_blocks": ref("PositiveInteger")},
            ["type", "target_blocks"]),
    ]}
    d["PwcError"] = obj({"code": ref("ErrorCode"), "message": {"type": "string"},
                         "details": {"type": "object"}, "retryable": {"type": "boolean"}},
                        ["code", "message"])
    limits = obj({k: ref("PositiveInteger") for k in [
        "max_page_size", "max_psbt_bytes", "max_raw_transaction_bytes", "max_message_bytes",
        "max_concurrent_operations", "operation_retention_seconds"]} | {
        "max_combine_psbts": {"type": "integer", "minimum": 2, "maximum": SAFE_INT},
        "high_fee_rate": ref("FeeRate")},
        ["max_page_size", "max_psbt_bytes", "max_combine_psbts", "max_raw_transaction_bytes",
         "max_message_bytes", "max_concurrent_operations", "operation_retention_seconds", "high_fee_rate"])
    d["Capabilities"] = obj({
        "pwc_version": ref("SemVer"),
        "provider": obj({"name": ref("Text"), "version": ref("Text"),
                         "platform": enum("desktop", "mobile", "extension", "other")},
                        ["name", "version", "platform"]),
        "profiles": arr(ref("ProfileName")),
        "supported_networks": arr(ref("Network")),
        "methods": arr(ref("MethodName")),
        "features": arr(ref("Feature")),
        "scopes": arr(ref("Scope")),
        "script_types": arr(ref("ScriptType")),
        "message_formats": arr(ref("MessageFormat")),
        "limits": limits,
    }, ["pwc_version", "provider", "profiles", "supported_networks", "methods", "features",
        "scopes", "script_types", "message_formats", "limits"])
    d["WalletSummary"] = obj({
        "wallet_id": ref("Id"), "display_name": ref("Text"), "network": ref("Network"),
        "wallet_kind": enum("software", "hardware", "multisig", "watch_only", "other"),
        "state": enum("ready", "locked", "syncing", "unavailable"),
        "account_ids": arr(ref("Id")),
    }, ["wallet_id", "display_name", "network", "wallet_kind", "state", "account_ids"],
        allOf=[{"if": {"properties": {"state": enum("locked", "unavailable")}},
                "then": {"properties": {"account_ids": {"maxItems": 0}}}}])
    d["AccountSummary"] = obj({
        "account_id": ref("Id"), "wallet_id": ref("Id"), "display_name": ref("Text"),
        "network": ref("Network"), "can_sign": {"type": "boolean"},
        "script_types": arr(ref("ScriptType")), "methods": arr(ref("MethodName")),
    }, ["account_id", "wallet_id", "display_name", "network", "can_sign", "script_types", "methods"],
        allOf=[{"if": {"properties": {"can_sign": {"const": False}}},
                "then": {"properties": {"methods": {"not": {"anyOf": [
                    {"contains": {"const": "psbt.sign"}}, {"contains": {"const": "message.sign"}}]}}}}}])
    sats_fields = ["confirmed_sats", "pending_self_sats", "pending_external_sats", "pending_outgoing_sats",
                   "spendable_sats", "locked_sats", "immature_sats"]
    d["Balance"] = obj({"account_id": ref("Id"), "network": ref("Network")}
                       | {k: ref("Sats") for k in sats_fields}
                       | {"unconfirmed_delta_sats": ref("SignedSats"), "updated_at": ref("Timestamp"),
                          "sync": ref("SyncStatus")},
                       ["account_id", "network", *sats_fields, "unconfirmed_delta_sats", "updated_at", "sync"])
    d["AddressRecord"] = obj({
        "account_id": ref("Id"), "network": ref("Network"), "address": ref("Address"),
        "role": enum("receive", "change", "unknown"), "state": enum("unused", "used", "unknown"),
        "label": ref("Text"),
    }, ["account_id", "network", "address", "role", "state"])
    confirmed_if = {"properties": {"confirmations": {"minimum": 1}}}
    d["Utxo"] = merge(d["OutPoint"], obj({
        "account_id": ref("Id"), "network": ref("Network"), "value_sats": ref("Sats"),
        "confirmations": ref("NonNegativeInteger"), "block_height": ref("BlockHeight"),
        "address": ref("Address"), "script_pub_key": ref("Hex"), "spendable": {"type": "boolean"},
        "restrictions": arr(enum("frozen", "locked", "immature", "unconfirmed", "unknown"), uniqueItems=True),
        "is_change": {"type": "boolean"},
    }, ["account_id", "network", "value_sats", "confirmations", "spendable", "restrictions"],
        allOf=[
            {"if": {"properties": {"spendable": {"const": True}}},
             "then": {"properties": {"restrictions": {"maxItems": 0}}},
             "else": {"properties": {"restrictions": {"minItems": 1}}}},
            {"if": confirmed_if, "then": {"required": ["block_height"]},
             "else": {"not": {"required": ["block_height"]}}},
        ]))
    d["TransactionRecord"] = obj({
        "account_id": ref("Id"), "network": ref("Network"), "txid": ref("Hash256"),
        "status": enum(*TX_STATUS), "confirmations": ref("NonNegativeInteger"),
        "block_height": ref("BlockHeight"), "first_seen_at": ref("Timestamp"),
        "confirmed_at": ref("Timestamp"), "net_change_sats": ref("SignedSats"),
        "fee_sats": ref("Sats"), "replaced_by_txid": ref("Hash256"),
    }, ["account_id", "network", "txid", "status", "confirmations", "net_change_sats"],
        allOf=[{"if": confirmed_if,
                "then": {"required": ["block_height"], "properties": {"status": {"const": "confirmed"}}},
                "else": {"not": {"required": ["block_height"]},
                         "properties": {"status": {"not": {"const": "confirmed"}}}}}])
    d["PsbtObject"] = obj({"psbt": ref("Base64"), "psbt_version": ref("PsbtVersion")}, ["psbt", "psbt_version"])
    d["AccountPsbtObject"] = merge(d["PsbtObject"], obj({"account_id": ref("Id"), "network": ref("Network")},
                                                        ["account_id", "network"]))
    d["CreatePsbtResult"] = merge(d["AccountPsbtObject"], obj({
        "fee_sats": ref("Sats"), "vsize": ref("PositiveInteger"),
        "change_output_indexes": arr(ref("UInt32"), uniqueItems=True),
        "reservation_id": ref("Id"), "reserved_until": ref("Timestamp"),
    }, ["fee_sats", "vsize", "change_output_indexes"], allOf=[
        {"if": {"required": ["reservation_id"]}, "then": {"required": ["reserved_until"]},
         "else": {"not": {"required": ["reserved_until"]}}}]))
    d["SignPsbtResult"] = merge(d["AccountPsbtObject"], obj({
        "signed_input_indexes": arr(ref("UInt32"), uniqueItems=True),
        "signing_status": enum("partial", "complete", "unknown"),
    }, ["signed_input_indexes", "signing_status"]))
    d["CombinePsbtResult"] = copy.deepcopy(d["PsbtObject"])
    d["FinalizePsbtResult"] = merge(d["PsbtObject"], obj({
        "complete": {"type": "boolean"}, "raw_transaction": ref("Hex"), "txid": ref("Hash256"),
    }, ["complete"], allOf=[
        {"if": {"properties": {"complete": {"const": True}}},
         "then": {"required": ["raw_transaction", "txid"]},
         "else": {"not": {"anyOf": [{"required": ["raw_transaction"]}, {"required": ["txid"]}]}}}]))
    d["GrantConstraints"] = obj({"max_amount_sats_per_request": ref("PositiveSats"),
                                 "valid_until": ref("Timestamp")}, [])
    d["Grant"] = obj({
        "grant_id": ref("Id"), "client_id": ref("Id"), "network": ref("Network"),
        "scopes": arr(ref("Scope"), uniqueItems=True), "wallet_ids": arr(ref("Id")),
        "account_ids": arr(ref("Id")), "created_at": ref("Timestamp"), "expires_at": ref("Timestamp"),
        "status": enum("active", "revoked", "expired"), "constraints": ref("GrantConstraints"),
    }, ["grant_id", "client_id", "network", "scopes", "wallet_ids", "account_ids", "created_at", "status"])
    d["SessionInfo"] = obj({
        "session_id": ref("Id"), "client_id": ref("Id"), "grant_ids": arr(ref("Id")),
        "created_at": ref("Timestamp"), "expires_at": ref("Timestamp"),
    }, ["session_id", "client_id", "grant_ids", "created_at"])
    d["Operation"] = obj({
        "operation_id": ref("Id"), "method": ref("MethodName"), "state": enum(*OP_STATES),
        "created_at": ref("Timestamp"), "updated_at": ref("Timestamp"), "expires_at": ref("Timestamp"),
        "account_id": ref("Id"), "result": {"type": "object"}, "error": ref("PwcError"),
    }, ["operation_id", "method", "state", "created_at", "updated_at"], allOf=[
        {"if": {"properties": {"state": enum("queued", "awaiting_approval", "awaiting_unlock", "awaiting_signer")}},
         "then": {"required": ["expires_at"]}},
        {"if": {"properties": {"state": {"not": {"const": "completed"}}}},
         "then": {"not": {"required": ["result"]}}},
        {"if": {"properties": {"state": enum("rejected", "expired", "canceled", "failed")}},
         "then": {"required": ["error"]}, "else": {"not": {"required": ["error"]}}},
        {"if": {"properties": {"state": {"const": "rejected"}}},
         "then": {"properties": {"error": {"properties": {"code": {"const": "USER_REJECTED"}}}}}},
        {"if": {"properties": {"state": {"const": "expired"}}},
         "then": {"properties": {"error": {"properties": {"code": {"const": "OPERATION_EXPIRED"}}}}}},
        {"if": {"properties": {"state": {"const": "canceled"}}},
         "then": {"properties": {"error": {"properties": {"code": {"const": "OPERATION_CANCELED"}}}}}},
    ])
    payloads = {
        "wallet.locked": obj({"wallet_id": ref("Id")}, ["wallet_id"]),
        "wallet.unlocked": obj({"wallet_id": ref("Id")}, ["wallet_id"]),
        "wallet.syncChanged": obj({"wallet_id": ref("Id"), "sync": ref("SyncStatus")}, ["wallet_id", "sync"]),
        "account.balanceChanged": obj({}, []),
        "account.transactionChanged": obj({"txid": ref("Hash256"), "status": enum(*TX_STATUS)}, ["txid", "status"]),
        "operation.stateChanged": obj({"state": enum(*OP_STATES)}, ["state"]),
        "grant.revoked": obj({"grant_id": ref("Id")}, ["grant_id"]),
        "grant.expired": obj({"grant_id": ref("Id")}, ["grant_id"]),
        "session.expired": obj({"session_id": ref("Id")}, ["session_id"]),
    }
    envelope_req = {"account.balanceChanged": ["account_id"], "account.transactionChanged": ["account_id"],
                    "operation.stateChanged": ["operation_id"]}
    rules = []
    for et, schema in payloads.items():
        then = {"properties": {"payload": schema}}
        if et in envelope_req:
            then["required"] = envelope_req[et]
        rules.append({"if": {"properties": {"event_type": {"const": et}}}, "then": then})
    d["PwcEvent"] = obj({
        "event_id": ref("Id"), "event_type": ref("EventType"), "occurred_at": ref("Timestamp"),
        "account_id": ref("Id"), "operation_id": ref("Id"), "payload": {"type": "object"},
    }, ["event_id", "event_type", "occurred_at", "payload"], allOf=rules)
    return d


def methods():
    """Return {method: (params_schema, direct_result_schema, completed_operation_result_or_None)}."""
    E = obj({}, [])
    page = lambda: obj({"account_id": ref("Id"), "cursor": ref("Cursor"), "page_size": ref("PositiveInteger")},
                       ["account_id"])
    listing = lambda key, item: obj({key: arr(ref(item)), "next_cursor": ref("Cursor")}, [key])
    grants = obj({"grants": arr(ref("Grant"))}, ["grants"])
    m = {
        "pwc.getCapabilities": (E, ref("Capabilities"), None),
        "session.getInfo": (E, ref("SessionInfo"), None),
        "session.requestPermissions": (obj({
            "scopes": arr(ref("Scope"), minItems=1, uniqueItems=True), "network": ref("Network"),
            "wallet_ids": arr(ref("Id")), "account_ids": arr(ref("Id")), "expires_at": ref("Timestamp"),
            "constraints": ref("GrantConstraints"), "idempotency_key": ref("Id"),
        }, ["scopes", "idempotency_key"]), ref("Operation"), grants),
        "session.getPermissions": (E, grants, None),
        "session.revokePermissions": (obj({"grant_id": ref("Id"), "idempotency_key": ref("Id")},
                                          ["grant_id", "idempotency_key"]),
                                      obj({"revoked": {"type": "boolean"}}, ["revoked"]), None),
        "wallet.list": (E, listing("wallets", "WalletSummary"), None),
        "account.list": (obj({"wallet_id": ref("Id")}, ["wallet_id"]), listing("accounts", "AccountSummary"), None),
        "account.getBalance": (obj({"account_id": ref("Id")}, ["account_id"]), ref("Balance"), None),
        "address.getReceiveAddress": (obj({"account_id": ref("Id")}, ["account_id"]), ref("AddressRecord"), None),
        "address.create": (obj({"account_id": ref("Id"), "label": ref("Text"), "idempotency_key": ref("Id")},
                               ["account_id", "idempotency_key"]), ref("AddressRecord"), None),
        "address.list": (page(), listing("addresses", "AddressRecord"), None),
        "utxo.list": (page(), listing("utxos", "Utxo"), None),
        "transaction.list": (page(), listing("transactions", "TransactionRecord"), None),
        "transaction.get": (obj({"account_id": ref("Id"), "txid": ref("Hash256")}, ["account_id", "txid"]),
                            obj({"transaction": ref("TransactionRecord")}, ["transaction"]), None),
        "psbt.create": (obj({
            "account_id": ref("Id"),
            "outputs": arr(obj({"address": ref("Address"), "amount_sats": ref("PositiveSats")},
                               ["address", "amount_sats"]), minItems=1),
            "fee_policy": ref("FeePolicy"), "selected_utxos": arr(ref("OutPoint")),
            "replaceable": {"type": "boolean"}, "idempotency_key": ref("Id"),
        }, ["account_id", "outputs", "fee_policy", "idempotency_key"]), ref("Operation"), ref("CreatePsbtResult")),
        "psbt.releaseReservation": (obj({"account_id": ref("Id"), "reservation_id": ref("Id"),
                                         "idempotency_key": ref("Id")},
                                        ["account_id", "reservation_id", "idempotency_key"]),
                                    obj({"released": {"type": "boolean"}}, ["released"]), None),
        "psbt.sign": (obj({"account_id": ref("Id"), "psbt": ref("Base64"),
                           "input_indexes": arr(ref("UInt32"), minItems=1, uniqueItems=True),
                           "idempotency_key": ref("Id")}, ["account_id", "psbt", "idempotency_key"]),
                      ref("Operation"), ref("SignPsbtResult")),
        "psbt.combine": (obj({"psbts": arr(ref("Base64"), minItems=2)}, ["psbts"]), ref("CombinePsbtResult"), None),
        "psbt.finalize": (obj({"psbt": ref("Base64")}, ["psbt"]), ref("FinalizePsbtResult"), None),
        "transaction.broadcast": (obj({"account_id": ref("Id"), "raw_transaction": ref("Hex"),
                                       "idempotency_key": ref("Id")},
                                      ["account_id", "raw_transaction", "idempotency_key"]),
                                  ref("Operation"), obj({"txid": ref("Hash256")}, ["txid"])),
        "message.sign": (obj({"account_id": ref("Id"), "address": ref("Address"), "message": {"type": "string"},
                              "format": ref("MessageFormat"), "idempotency_key": ref("Id")},
                             ["account_id", "address", "message", "format", "idempotency_key"]),
                         ref("Operation"),
                         obj({"signature": ref("Base64"), "format": ref("MessageFormat"), "address": ref("Address")},
                             ["signature", "format", "address"])),
        "message.verify": (obj({"network": ref("Network"), "address": ref("Address"), "message": {"type": "string"},
                                "signature": ref("Base64"), "format": ref("MessageFormat")},
                               ["network", "address", "message", "signature", "format"]),
                           obj({"valid": {"type": "boolean"}}, ["valid"]), None),
        "operation.get": (obj({"operation_id": ref("Id")}, ["operation_id"]), ref("Operation"), None),
        "operation.cancel": (obj({"operation_id": ref("Id"), "idempotency_key": ref("Id")},
                                 ["operation_id", "idempotency_key"]), ref("Operation"), None),
    }
    return m


def add_methods(defs, mtable):
    index = {}
    for name, (params, result, op_result) in mtable.items():
        defs[f"{name}.params"] = params
        defs[f"{name}.result"] = result
        entry = {"params": {"$ref": f"#/$defs/{name}.params"}, "result": {"$ref": f"#/$defs/{name}.result"}}
        if op_result is not None:
            defs[f"{name}.operationResult"] = op_result
            entry["operationResult"] = {"$ref": f"#/$defs/{name}.operationResult"}
        index[name] = entry
    return index


def strictify(node):
    if isinstance(node, dict):
        if node.get("type") == "object" and "properties" in node:
            node["additionalProperties"] = False
        for v in node.values():
            strictify(v)
    elif isinstance(node, list):
        for v in node:
            strictify(v)


def core_schema(strict=False):
    defs = primitives() | composites()
    index = add_methods(defs, methods())
    s = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": CORE_STRICT_ID if strict else CORE_ID,
        "title": "PWC Core v0.2.1" + (" (strict)" if strict else ""),
        "description": ("Machine-readable companion of PWC-CORE-001 v0.2.1. If this schema and the specification "
                        "text disagree, the text prevails (Core §3.2). x-pwc-methods maps every Core method to its "
                        "params, direct result and, for Operation-returning methods, the completed Operation result."
                        + (" Strict variant: unknown members are rejected; use it to test Provider output."
                           if strict else " Unknown members are allowed (Core §3.1).")),
        "x-pwc-methods": index,
        "$defs": defs,
    }
    if strict:
        strictify(s["$defs"])
    return s


def local_schema():
    core = lambda name: {"$ref": f"{CORE_ID}#/$defs/{name}"}
    hex32 = {"type": "string", "pattern": r"^[0-9a-f]{64}$"}
    rpc_id = {"oneOf": [{"type": "string", "minLength": 1, "maxLength": 64},
                        {"type": "integer", "minimum": -SAFE_INT, "maximum": SAFE_INT}]}
    defs = {
        "RpcId": rpc_id,
        "Hex32": hex32,
        "Request": obj({"jsonrpc": {"const": "2.0"}, "id": ref("RpcId"), "method": core("MethodName"),
                        "params": {"type": "object"}}, ["jsonrpc", "id", "method", "params"],
                       additionalProperties=False),
        "SuccessResponse": obj({"jsonrpc": {"const": "2.0"}, "id": ref("RpcId"), "result": {"type": "object"}},
                               ["jsonrpc", "id", "result"], additionalProperties=False),
        "ErrorResponse": obj({
            "jsonrpc": {"const": "2.0"},
            "id": {"oneOf": [ref("RpcId"), {"type": "null"}]},
            "error": {"oneOf": [
                obj({"code": {"const": 1000}, "message": {"type": "string"}, "data": core("PwcError")},
                    ["code", "message", "data"], additionalProperties=False),
                obj({"code": {"enum": [-32700, -32600]}, "message": {"type": "string"}},
                    ["code", "message"], additionalProperties=False),
            ]},
        }, ["jsonrpc", "id", "error"], additionalProperties=False),
        "EventNotification": obj({"jsonrpc": {"const": "2.0"}, "method": {"const": "pwc.event"},
                                  "params": core("PwcEvent")}, ["jsonrpc", "method", "params"],
                                 additionalProperties=False),
        "DelegatedEventNotification": obj({"jsonrpc": {"const": "2.0"}, "method": {"const": "local.delegatedEvent"},
                                           "params": obj({"delegate_id": core("Id"), "event": core("PwcEvent")},
                                                         ["delegate_id", "event"])},
                                          ["jsonrpc", "method", "params"], additionalProperties=False),
        "DelegatePrincipal": obj({"kind": {"const": "web_origin"},
                                  "origin": {"type": "string",
                                             "pattern": r"^(https://[^/?#\s]+|http://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]{1,5})?)$"},
                                  "private": {"type": "boolean"}}, ["kind", "origin", "private"]),
        "ProviderFrame": {"oneOf": [ref("SuccessResponse"), ref("ErrorResponse"), ref("EventNotification"),
                                    ref("DelegatedEventNotification")]},
    }
    mtable = {
        "local.hello": (obj({"protocols": arr({"type": "string", "minLength": 1}, minItems=1)}, ["protocols"]),
                        obj({"protocol": {"type": "string"}, "pwc_version": core("SemVer"),
                             "provider_id": {"type": "string", "pattern": r"^[a-z0-9-]{1,32}$"},
                             "provider": obj({"name": core("Text"), "version": core("Text")}, ["name", "version"]),
                             "max_frame_bytes": {"type": "integer", "minimum": 1048576, "maximum": SAFE_INT},
                             "server_nonce": ref("Hex32"), "delegation": {"type": "boolean"}},
                            ["protocol", "pwc_version", "provider_id", "provider", "max_frame_bytes", "server_nonce",
                             "delegation"]),
                        None),
        "local.pair": (obj({"client_name": core("Text"), "client_description": core("Text"),
                            "pairing_code": {"type": "string", "pattern": r"^[0-9]{6}$"},
                            "delegator": {"type": "boolean"}},
                           ["client_name", "pairing_code"]),
                       obj({"client_id": core("Id"), "client_secret": ref("Hex32"), "session_id": core("Id")},
                           ["client_id", "client_secret", "session_id"]), None),
        "local.authenticate": (obj({"client_id": core("Id"), "client_nonce": ref("Hex32"),
                                    "client_proof": ref("Hex32")}, ["client_id", "client_nonce", "client_proof"]),
                               obj({"session_id": core("Id"), "server_proof": ref("Hex32")},
                                   ["session_id", "server_proof"]), None),
        "local.unpair": (obj({}, []), obj({"unpaired": {"const": True}}, ["unpaired"]), None),
        "local.openDelegate": (obj({"principal": ref("DelegatePrincipal")}, ["principal"]),
                               obj({"delegate_id": core("Id"), "client_id": core("Id"), "session_id": core("Id")},
                                   ["delegate_id", "client_id", "session_id"]), None),
        "local.delegated": (obj({"delegate_id": core("Id"),
                                 "method": {"allOf": [core("MethodName"), {"not": {"pattern": "^local\\."}}]},
                                 "params": {"type": "object"}}, ["delegate_id", "method", "params"]),
                            {"type": "object"}, None),
        "local.closeDelegate": (obj({"delegate_id": core("Id")}, ["delegate_id"]),
                                obj({"closed": {"const": True}}, ["closed"]), None),
    }
    index = add_methods(defs, mtable)
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": LOCAL_ID,
        "title": "PWC Transport Profile LOCAL v0.1.0",
        "description": ("Frames and local.* methods of PWC-TRANSPORT-LOCAL-001 v0.1.0. Core types are referenced "
                        "from the Core schema, which must be registered with the validator."),
        "x-pwc-methods": index,
        "$defs": defs,
    }


def browser_schema():
    core = lambda name: {"$ref": f"{CORE_ID}#/$defs/{name}"}
    defs = {
        "PwcProviderInfo": obj({
            "uuid": {"type": "string",
                     "pattern": r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"},
            "provider_id": {"type": "string", "pattern": r"^[a-z0-9-]{1,32}$"},
            "name": core("Text"),
            "icon": {"type": "string", "pattern": r"^data:image/(png|webp|svg\+xml)[;,]"},
            "protocol": {"const": "pwc-browser/1"},
        }, ["uuid", "provider_id", "name", "icon", "protocol"]),
        "RequestArguments": obj({"method": core("MethodName"), "params": {"type": "object"}}, ["method"]),
        "RejectionError": merge(
            obj({"name": {"const": "PwcError"}}, ["name"]),
            obj({"code": core("ErrorCode"), "message": {"type": "string"}, "details": {"type": "object"},
                 "retryable": {"type": "boolean"}}, ["code", "message"])),
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": BROWSER_ID,
        "title": "PWC Transport Profile BROWSER v0.1.0",
        "description": ("JSON-representable values of PWC-TRANSPORT-BROWSER-001 v0.1.0: the announced provider info, "
                        "the request arguments and the members of a rejection error. The provider object itself is a "
                        "JavaScript object and is not described here."),
        "$defs": defs,
    }


def remote_schema():
    core = lambda name: {"$ref": f"{CORE_ID}#/$defs/{name}"}
    local = lambda name: {"$ref": f"{LOCAL_ID}#/$defs/{name}"}
    hexn = lambda n: {"type": "string", "pattern": rf"^[0-9a-f]{{{2 * n}}}$"}
    wss = {"type": "string", "pattern": r"^wss://[^\s]+$"}
    relays = arr(wss, minItems=1, uniqueItems=True)
    defs = {
        "PublicKey": hexn(32),
        "WssUrl": wss,
        "NostrEvent": obj({
            "id": hexn(32), "pubkey": hexn(32), "created_at": core("NonNegativeInteger"),
            "kind": {"const": 27180},
            "tags": {"type": "array", "minItems": 1, "maxItems": 1,
                     "items": {"type": "array", "prefixItems": [{"const": "p"}, hexn(32)],
                               "minItems": 2, "maxItems": 2}},
            "content": {"type": "string", "minLength": 1},
            "sig": hexn(64),
        }, ["id", "pubkey", "created_at", "kind", "tags", "content", "sig"]),
        "RemotePart": obj({"pwc_part": obj({
            "message_id": hexn(16), "index": core("UInt32"),
            "count": {"type": "integer", "minimum": 2, "maximum": 4294967295}, "data": core("Base64"),
        }, ["message_id", "index", "count", "data"], additionalProperties=False)}, ["pwc_part"],
            additionalProperties=False),
        "RemotePairedParams": obj({
            "secret": hexn(16), "client_id": core("Id"), "session_id": core("Id"), "pwc_version": core("SemVer"),
            "provider_id": {"type": "string", "pattern": r"^[a-z0-9-]{1,32}$"},
            "provider": obj({"name": core("Text"), "version": core("Text")}, ["name", "version"]),
            "relays": relays,
            "max_message_bytes": {"type": "integer", "minimum": 1048576, "maximum": SAFE_INT},
            "wake_url": {"type": "string", "minLength": 1},
        }, ["secret", "client_id", "session_id", "pwc_version", "provider_id", "provider", "relays",
            "max_message_bytes"]),
        "Notification": {"oneOf": [
            local("EventNotification"),
            obj({"jsonrpc": {"const": "2.0"}, "method": {"const": "remote.paired"},
                 "params": ref("RemotePairedParams")}, ["jsonrpc", "method", "params"], additionalProperties=False),
            obj({"jsonrpc": {"const": "2.0"}, "method": {"const": "remote.pairingRejected"},
                 "params": obj({"secret": hexn(16)}, ["secret"])}, ["jsonrpc", "method", "params"],
                additionalProperties=False),
            obj({"jsonrpc": {"const": "2.0"}, "method": {"const": "remote.relaysChanged"},
                 "params": obj({"relays": relays}, ["relays"])}, ["jsonrpc", "method", "params"],
                additionalProperties=False),
        ]},
        "Plaintext": {"oneOf": [local("Request"), local("SuccessResponse"), local("ErrorResponse"),
                                ref("Notification"), ref("RemotePart")]},
    }
    mtable = {
        "remote.ping": (obj({}, []), obj({}, []), None),
        "remote.updateRelays": (obj({"relays": relays}, ["relays"]), obj({}, []), None),
        "remote.unpair": (obj({}, []), obj({"unpaired": {"const": True}}, ["unpaired"]), None),
    }
    index = add_methods(defs, mtable)
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": REMOTE_ID,
        "title": "PWC Transport Profile REMOTE v0.1.0",
        "description": ("Nostr event envelope, decrypted plaintexts and remote.* methods of PWC-TRANSPORT-REMOTE-001 "
                        "v0.1.0. Requires the Core and LOCAL schemas to be registered with the validator."),
        "x-pwc-methods": index,
        "$defs": defs,
    }


def write(name, schema):
    (HERE / name).write_text(json.dumps(schema, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    write("pwc-core-0.2.1.schema.json", core_schema())
    write("pwc-core-0.2.1.strict.schema.json", core_schema(strict=True))
    write("pwc-transport-local-0.1.0.schema.json", local_schema())
    write("pwc-transport-browser-0.1.0.schema.json", browser_schema())
    write("pwc-transport-remote-0.1.0.schema.json", remote_schema())
    print("written")
