"""Reference computation of a Core Balance snapshot (PWC-CORE-001 v0.2.1 §4.3).

A scenario describes the account's confirmed outputs and its active pending projection:

  confirmed_outputs:    [{outpoint, value, immature?, locked?}]
  pending_transactions: [{txid, inputs: [outpoint], outputs: [{outpoint, value, owned, locked?}]}]
  policy:               {spend_unconfirmed_self: bool, spend_unconfirmed_external: bool}

An input not listed among the account's confirmed or pending outputs is a foreign input.
"""


def compute(s):
    confirmed = {o["outpoint"]: o for o in s["confirmed_outputs"]}
    pending = s["pending_transactions"]
    produced = {}                               # outpoint -> (tx, output) for owned pending outputs
    for tx in pending:
        for out in tx["outputs"]:
            if out["owned"]:
                produced[out["outpoint"]] = (tx, out)
    spent = {i for tx in pending for i in tx["inputs"]}
    by_txid = {tx["txid"]: tx for tx in pending}

    def self_funded(tx, seen=()):
        for i in tx["inputs"]:
            if i in confirmed:
                continue
            if i in produced:
                parent = produced[i][0]
                if parent["txid"] in seen or not self_funded(parent, seen + (tx["txid"],)):
                    return False
                continue
            return False                        # foreign input
        return True

    C = sum(o["value"] for o in confirmed.values())
    D = sum(o["value"] for op, o in confirmed.items() if op in spent)
    p_self = p_ext = 0
    spendable = locked = immature = 0
    for op, o in confirmed.items():
        if op in spent:
            continue
        if o.get("immature"):
            immature += o["value"]
        elif o.get("locked"):
            locked += o["value"]
        else:
            spendable += o["value"]
    for op, (tx, out) in produced.items():
        if op in spent:
            continue
        is_self = self_funded(by_txid[tx["txid"]])
        if is_self:
            p_self += out["value"]
        else:
            p_ext += out["value"]
        if out.get("locked"):
            locked += out["value"]
        elif (s["policy"]["spend_unconfirmed_self"] if is_self else s["policy"]["spend_unconfirmed_external"]):
            spendable += out["value"]
    delta = p_self + p_ext - D
    assert C + delta >= 0
    assert spendable + locked + immature <= C + delta
    return {"confirmed_sats": str(C), "pending_self_sats": str(p_self), "pending_external_sats": str(p_ext),
            "pending_outgoing_sats": str(D), "unconfirmed_delta_sats": str(delta), "spendable_sats": str(spendable),
            "locked_sats": str(locked), "immature_sats": str(immature)}
