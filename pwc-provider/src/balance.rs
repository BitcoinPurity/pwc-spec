//! Core §4.3 balance snapshot, computed from the account's confirmed outputs and its pending projection.
//! The input format is that of `pwc-testkit/vectors/balance.json`.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::types::{Sats, SignedSats};

#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    /// Account-owned outputs unspent on the confirmed chain alone (`C`).
    pub confirmed_outputs: Vec<ConfirmedOutput>,
    /// The active, nonconflicting pending projection.
    pub pending_transactions: Vec<PendingTx>,
    pub policy: Policy,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfirmedOutput {
    pub outpoint: String,
    pub value: u64,
    #[serde(default)]
    pub immature: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PendingTx {
    pub txid: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<PendingOutput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PendingOutput {
    pub outpoint: String,
    pub value: u64,
    pub owned: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Policy {
    pub spend_unconfirmed_self: bool,
    pub spend_unconfirmed_external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Components {
    pub confirmed_sats: Sats,
    pub pending_self_sats: Sats,
    pub pending_external_sats: Sats,
    pub pending_outgoing_sats: Sats,
    pub unconfirmed_delta_sats: SignedSats,
    pub spendable_sats: Sats,
    pub locked_sats: Sats,
    pub immature_sats: Sats,
}

/// How an effective unspent output counts (Core §4.3, §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Spendable,
    Locked,
    Immature,
    /// Unconfirmed and refused by the spending policy.
    Unconfirmed,
}

#[derive(Debug, Clone)]
pub struct Effective {
    pub outpoint: String,
    pub value: u64,
    pub confirmed: bool,
    pub class: Class,
}

pub struct Snapshot {
    pub components: Components,
    /// Every effective unspent output `(C − D) ∪ P`.
    pub outputs: Vec<Effective>,
}

pub fn compute(s: &Scenario) -> Snapshot {
    let confirmed: HashSet<&str> = s.confirmed_outputs.iter().map(|o| o.outpoint.as_str()).collect();
    let mut producer: HashMap<&str, usize> = HashMap::new();
    for (i, tx) in s.pending_transactions.iter().enumerate() {
        for out in tx.outputs.iter().filter(|o| o.owned) {
            producer.insert(out.outpoint.as_str(), i);
        }
    }
    let spent: HashSet<&str> = s.pending_transactions.iter().flat_map(|t| t.inputs.iter().map(String::as_str)).collect();

    let mut memo: HashMap<usize, bool> = HashMap::new();
    fn self_funded<'a>(
        i: usize,
        s: &'a Scenario,
        confirmed: &HashSet<&str>,
        producer: &HashMap<&str, usize>,
        memo: &mut HashMap<usize, bool>,
    ) -> bool {
        if let Some(v) = memo.get(&i) {
            return *v;
        }
        memo.insert(i, false);
        let v = s.pending_transactions[i].inputs.iter().all(|input| {
            confirmed.contains(input.as_str())
                || producer.get(input.as_str()).is_some_and(|p| self_funded(*p, s, confirmed, producer, memo))
        });
        memo.insert(i, v);
        v
    }

    let mut c = 0u64;
    let mut d = 0u64;
    let (mut p_self, mut p_ext) = (0u64, 0u64);
    let (mut spendable, mut locked, mut immature) = (0u64, 0u64, 0u64);
    let mut outputs = Vec::new();
    for o in &s.confirmed_outputs {
        c += o.value;
        if spent.contains(o.outpoint.as_str()) {
            d += o.value;
            continue;
        }
        let class = if o.immature {
            immature += o.value;
            Class::Immature
        } else if o.locked {
            locked += o.value;
            Class::Locked
        } else {
            spendable += o.value;
            Class::Spendable
        };
        outputs.push(Effective { outpoint: o.outpoint.clone(), value: o.value, confirmed: true, class });
    }
    for (i, tx) in s.pending_transactions.iter().enumerate() {
        for out in tx.outputs.iter().filter(|o| o.owned && !spent.contains(o.outpoint.as_str())) {
            let is_self = self_funded(i, s, &confirmed, &producer, &mut memo);
            if is_self {
                p_self += out.value;
            } else {
                p_ext += out.value;
            }
            let allowed = if is_self { s.policy.spend_unconfirmed_self } else { s.policy.spend_unconfirmed_external };
            let class = if out.locked {
                locked += out.value;
                Class::Locked
            } else if allowed {
                spendable += out.value;
                Class::Spendable
            } else {
                Class::Unconfirmed
            };
            outputs.push(Effective { outpoint: out.outpoint.clone(), value: out.value, confirmed: false, class });
        }
    }
    let delta = p_self as i64 + p_ext as i64 - d as i64;
    let components = Components {
        confirmed_sats: Sats(c),
        pending_self_sats: Sats(p_self),
        pending_external_sats: Sats(p_ext),
        pending_outgoing_sats: Sats(d),
        unconfirmed_delta_sats: SignedSats(delta),
        spendable_sats: Sats(spendable),
        locked_sats: Sats(locked),
        immature_sats: Sats(immature),
    };
    Snapshot { components, outputs }
}
