//! In-memory demo wallet and simulated chains. Keys derive from fixed seeds: test use only.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

use bitcoin::hashes::{sha256, Hash};
use bitcoin::secp256k1::{All, Secp256k1, SecretKey};
use bitcoin::{
    absolute, transaction, Address, Amount, CompressedPublicKey, OutPoint, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Txid, Witness,
};

use crate::backend::*;
use crate::balance::{self, Class, ConfirmedOutput, PendingOutput, PendingTx, Policy, Scenario};
use crate::error::{err, ErrorCode, PwcError};
use crate::types::{
    chain_network, Hash256, Hex, Id, MessageFormat, Sats, ScriptType, SignedSats, Text, Timestamp, SUPPORTED_NETWORKS,
};

/// Account-scoped methods (Core §4.2) by account kind.
const SIGNING_WPKH: &[&str] = &[
    "account.getBalance",
    "address.getReceiveAddress",
    "address.create",
    "address.list",
    "utxo.list",
    "transaction.list",
    "transaction.get",
    "psbt.create",
    "psbt.releaseReservation",
    "psbt.sign",
    "transaction.broadcast",
    "message.sign",
];
const SIGNING_PKH: &[&str] = &[
    "account.getBalance",
    "address.getReceiveAddress",
    "address.create",
    "address.list",
    "utxo.list",
    "transaction.list",
    "transaction.get",
    "transaction.broadcast",
    "message.sign",
];
const WATCH_WPKH: &[&str] = &[
    "account.getBalance",
    "address.getReceiveAddress",
    "address.create",
    "address.list",
    "utxo.list",
    "transaction.list",
    "transaction.get",
    "psbt.create",
    "psbt.releaseReservation",
    "transaction.broadcast",
];

const COINBASE_MATURITY: u32 = 100;
const MIN_FEE_RATE: u64 = 1000;
const DUST: u64 = 294;
const POLICY: Policy = Policy {
    spend_unconfirmed_self: true,
    spend_unconfirmed_external: false,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Receive,
    Change,
}

struct Key {
    secret: SecretKey,
    public: CompressedPublicKey,
    script: ScriptBuf,
    address: Address,
    role: Role,
    label: Option<Text>,
}

struct Account {
    info: AccountInfo,
    keys: Vec<Key>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TxStatus {
    Confirmed { height: u32 },
    Pending,
    Replaced { by: Txid },
    Dropped,
}

struct ChainTx {
    tx: Transaction,
    status: TxStatus,
    coinbase: bool,
    first_seen: Timestamp,
    confirmed_at: Option<Timestamp>,
}

struct Chain {
    tip: u32,
    txs: Vec<ChainTx>,
    offline: bool,
    fee_estimate: Option<u64>,
}

struct Mem {
    wallets: Vec<WalletInfo>,
    accounts: BTreeMap<Id, Account>,
    chains: HashMap<String, Chain>,
    foreign: u64,
}

pub struct MemoryBackend {
    mem: Mutex<Mem>,
    secp: Secp256k1<All>,
}

fn derive_key(
    secp: &Secp256k1<All>,
    account_id: &str,
    role: Role,
    index: usize,
    script_type: ScriptType,
    network: &str,
) -> Key {
    let tag = if role == Role::Receive { "receive" } else { "change" };
    let seed = sha256::Hash::hash(format!("pwc-provider demo key|{account_id}|{tag}|{index}").as_bytes());
    let secret = SecretKey::from_slice(seed.as_byte_array()).expect("valid demo key");
    let public = CompressedPublicKey(secret.public_key(secp));
    let net = chain_network(network).expect("supported network");
    let address = match script_type {
        ScriptType::P2wpkh => Address::p2wpkh(&public, net),
        ScriptType::P2pkh => Address::p2pkh(public, net),
    };
    Key {
        secret,
        public,
        script: address.script_pubkey(),
        address,
        role,
        label: None,
    }
}

fn hash256(txid: &Txid) -> Hash256 {
    Hash256(txid.to_string())
}

fn outpoint_key(o: &OutPoint) -> String {
    format!("{}:{}", o.txid, o.vout)
}

impl Mem {
    fn account(&self, account_id: &str) -> &Account {
        self.accounts.get(account_id).expect("existing account")
    }

    fn chain(&self, network: &str) -> &Chain {
        &self.chains[network]
    }

    fn owns(&self, account: &Account, script: &bitcoin::Script) -> Option<usize> {
        account.keys.iter().position(|k| k.script.as_script() == script)
    }
}

impl Chain {
    /// Confirmed transactions and the active pending projection: pending transactions none of whose
    /// unconfirmed ancestors were replaced, conflicted or dropped.
    fn active(&self) -> Vec<usize> {
        let mut active: Vec<usize> = Vec::new();
        let mut index: HashMap<Txid, usize> = HashMap::new();
        for (i, t) in self.txs.iter().enumerate() {
            index.insert(t.tx.compute_txid(), i);
        }
        let mut ok: HashSet<usize> = HashSet::new();
        for (i, t) in self.txs.iter().enumerate() {
            let live = match t.status {
                TxStatus::Confirmed { .. } => true,
                TxStatus::Pending => t.tx.input.iter().all(|inp| match index.get(&inp.previous_output.txid) {
                    Some(p) => ok.contains(p),
                    None => true,
                }),
                _ => false,
            };
            if live {
                ok.insert(i);
                active.push(i);
            }
        }
        active
    }

    fn confirmations(&self, status: TxStatus) -> u32 {
        match status {
            TxStatus::Confirmed { height } => self.tip.saturating_sub(height) + 1,
            _ => 0,
        }
    }

    fn output(&self, o: &OutPoint) -> Option<&TxOut> {
        self.txs
            .iter()
            .find(|t| t.tx.compute_txid() == o.txid)
            .and_then(|t| t.tx.output.get(o.vout as usize))
    }
}

/// An effective unspent output of an account, with its chain context.
struct Coin {
    outpoint: OutPoint,
    txout: TxOut,
    key: usize,
    confirmations: u32,
    height: Option<u32>,
    class: Class,
}

impl MemoryBackend {
    /// The demo data of `docs/api-contract.md` §6.
    pub fn demo() -> MemoryBackend {
        let backend = MemoryBackend {
            mem: Mutex::new(Mem {
                wallets: Vec::new(),
                accounts: BTreeMap::new(),
                chains: HashMap::new(),
                foreign: 0,
            }),
            secp: Secp256k1::new(),
        };
        for net in SUPPORTED_NETWORKS {
            backend.lock().chains.insert(
                net.to_string(),
                Chain {
                    tip: 1000,
                    txs: Vec::new(),
                    offline: false,
                    fee_estimate: Some(2000),
                },
            );
            let main = backend.add_wallet(&format!("w-{net}-main"), "Main wallet", net, WalletKind::Software);
            let wpkh = backend.add_account(&main, &format!("a-{net}-wpkh"), "Savings", ScriptType::P2wpkh, true);
            let pkh = backend.add_account(&main, &format!("a-{net}-pkh"), "Legacy", ScriptType::P2pkh, true);
            let watch = backend.add_wallet(
                &format!("w-{net}-watch"),
                "Watch-only wallet",
                net,
                WalletKind::WatchOnly,
            );
            let cold = backend.add_account(
                &watch,
                &format!("a-{net}-watch"),
                "Cold storage",
                ScriptType::P2wpkh,
                false,
            );
            backend.receive(&wpkh, 1_000_000, 6);
            backend.receive(&wpkh, 500_000, 6);
            backend.receive(&pkh, 200_000, 6);
            backend.receive(&cold, 300_000, 6);
        }
        backend
    }

    fn lock(&self) -> MutexGuard<'_, Mem> {
        self.mem.lock().unwrap()
    }

    pub fn add_wallet(&self, wallet_id: &str, name: &str, network: &str, kind: WalletKind) -> Id {
        let id = Id(wallet_id.into());
        self.lock().wallets.push(WalletInfo {
            wallet_id: id.clone(),
            display_name: Text(name.into()),
            network: network.into(),
            kind,
            state: WalletState::Ready,
            account_ids: Vec::new(),
        });
        id
    }

    pub fn add_account(
        &self,
        wallet_id: &Id,
        account_id: &str,
        name: &str,
        script_type: ScriptType,
        can_sign: bool,
    ) -> Id {
        let mut mem = self.lock();
        let wallet = mem
            .wallets
            .iter_mut()
            .find(|w| &w.wallet_id == wallet_id)
            .expect("known wallet");
        let id = Id(account_id.into());
        wallet.account_ids.push(id.clone());
        let network = wallet.network.clone();
        let methods = match (script_type, can_sign) {
            (ScriptType::P2wpkh, true) => SIGNING_WPKH,
            (ScriptType::P2pkh, true) => SIGNING_PKH,
            (ScriptType::P2wpkh, false) => WATCH_WPKH,
            (ScriptType::P2pkh, false) => &SIGNING_PKH[..7],
        };
        let mut keys: Vec<Key> = (0..3)
            .map(|i| derive_key(&self.secp, account_id, Role::Receive, i, script_type, &network))
            .collect();
        keys.push(derive_key(
            &self.secp,
            account_id,
            Role::Change,
            0,
            script_type,
            &network,
        ));
        let info = AccountInfo {
            account_id: id.clone(),
            wallet_id: wallet_id.clone(),
            display_name: Text(name.into()),
            network,
            can_sign,
            script_type,
            methods: methods.to_vec(),
        };
        mem.accounts.insert(id.clone(), Account { info, keys });
        id
    }

    pub fn set_wallet_state(&self, wallet_id: &str, state: WalletState) {
        let mut mem = self.lock();
        let wallet = mem
            .wallets
            .iter_mut()
            .find(|w| w.wallet_id.as_str() == wallet_id)
            .expect("known wallet");
        wallet.state = state;
    }

    fn foreign_input(mem: &mut Mem) -> TxIn {
        mem.foreign += 1;
        let txid =
            Txid::from_byte_array(sha256::Hash::hash(format!("foreign {}", mem.foreign).as_bytes()).to_byte_array());
        TxIn {
            previous_output: OutPoint { txid, vout: 0 },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }
    }

    fn add_tx(mem: &mut Mem, network: &str, tx: Transaction, confirmations: u32, coinbase: bool) -> Txid {
        let chain = mem.chains.get_mut(network).expect("supported network");
        let txid = tx.compute_txid();
        let now = Timestamp::now();
        let status = if confirmations == 0 {
            TxStatus::Pending
        } else {
            TxStatus::Confirmed {
                height: chain.tip + 1 - confirmations,
            }
        };
        let confirmed_at = (confirmations > 0).then_some(now);
        chain.txs.push(ChainTx {
            tx,
            status,
            coinbase,
            first_seen: now,
            confirmed_at,
        });
        txid
    }

    fn payment(mem: &mut Mem, account_id: &str, value: u64, input: TxIn) -> (String, Transaction) {
        let account = mem.account(account_id);
        let script = account.keys[0].script.clone();
        let network = account.info.network.clone();
        let tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![input],
            output: vec![TxOut {
                value: Amount::from_sat(value),
                script_pubkey: script,
            }],
        };
        (network, tx)
    }

    /// A payment from a third party to the account's first receive address, with `confirmations` (0: pending).
    pub fn receive(&self, account_id: &Id, value: u64, confirmations: u32) -> Txid {
        let mut mem = self.lock();
        let input = Self::foreign_input(&mut mem);
        let (network, tx) = Self::payment(&mut mem, account_id.as_str(), value, input);
        Self::add_tx(&mut mem, &network, tx, confirmations, false)
    }

    /// A coinbase output to the account.
    pub fn receive_coinbase(&self, account_id: &Id, value: u64, confirmations: u32) -> Txid {
        let mut mem = self.lock();
        let input = TxIn {
            previous_output: OutPoint::null(),
            ..Self::foreign_input(&mut mem)
        };
        let (network, tx) = Self::payment(&mut mem, account_id.as_str(), value, input);
        Self::add_tx(&mut mem, &network, tx, confirmations, true)
    }

    /// A pending payment to the account that double-spends the input of the pending payment `txid`.
    pub fn receive_conflicting(&self, account_id: &Id, txid: &Txid, value: u64) -> Txid {
        let mut mem = self.lock();
        let network = mem.account(account_id.as_str()).info.network.clone();
        let input = mem
            .chain(&network)
            .txs
            .iter()
            .find(|t| &t.tx.compute_txid() == txid)
            .expect("known tx")
            .tx
            .input[0]
            .clone();
        let (network, tx) = Self::payment(&mut mem, account_id.as_str(), value, input);
        Self::add_tx(&mut mem, &network, tx, 0, false)
    }

    fn with_tx(&self, network: &str, txid: &Txid, f: impl FnOnce(&mut ChainTx)) {
        let mut mem = self.lock();
        let chain = mem.chains.get_mut(network).expect("supported network");
        f(chain
            .txs
            .iter_mut()
            .find(|t| &t.tx.compute_txid() == txid)
            .expect("known tx"));
    }

    /// The wallet accepts `by` as replacement of the pending transaction `txid`.
    pub fn replace(&self, network: &str, txid: &Txid, by: &Txid) {
        let by = *by;
        self.with_tx(network, txid, |t| t.status = TxStatus::Replaced { by });
    }

    pub fn drop_pending(&self, network: &str, txid: &Txid) {
        self.with_tx(network, txid, |t| t.status = TxStatus::Dropped);
    }

    /// Mines `blocks` blocks; the first confirms every active pending transaction.
    pub fn mine(&self, network: &str, blocks: u32) {
        let mut mem = self.lock();
        let chain = mem.chains.get_mut(network).expect("supported network");
        let active = chain.active();
        let height = chain.tip + 1;
        chain.tip += blocks;
        let now = Timestamp::now();
        for i in active {
            if chain.txs[i].status == TxStatus::Pending {
                chain.txs[i].status = TxStatus::Confirmed { height };
                chain.txs[i].confirmed_at = Some(now);
            }
        }
    }

    /// A reorganization removes the last `depth` blocks; their transactions return to pending.
    pub fn reorg(&self, network: &str, depth: u32) {
        let mut mem = self.lock();
        let chain = mem.chains.get_mut(network).expect("supported network");
        chain.tip -= depth;
        let tip = chain.tip;
        for t in chain.txs.iter_mut() {
            if matches!(t.status, TxStatus::Confirmed { height } if height > tip) {
                t.status = TxStatus::Pending;
                t.confirmed_at = None;
            }
        }
    }

    pub fn set_offline(&self, network: &str, offline: bool) {
        self.lock().chains.get_mut(network).expect("supported network").offline = offline;
    }

    pub fn set_fee_estimate(&self, network: &str, sat_per_kvb: Option<u64>) {
        self.lock()
            .chains
            .get_mut(network)
            .expect("supported network")
            .fee_estimate = sat_per_kvb;
    }

    /// The User spends an output of the account from the wallet's own interface, paying `value` to a third
    /// party; the transaction enters the pending projection.
    pub fn spend_by_user(&self, account_id: &Id, outpoint: &OutPoint, value: u64) -> Txid {
        let mut mem = self.lock();
        let network = mem.account(account_id.as_str()).info.network.clone();
        let input = TxIn {
            previous_output: *outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };
        let tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![input],
            output: vec![TxOut {
                value: Amount::from_sat(value),
                script_pubkey: ScriptBuf::new_op_return([]),
            }],
        };
        Self::add_tx(&mut mem, &network, tx, 0, false)
    }

    pub fn tip(&self, network: &str) -> u32 {
        self.lock().chain(network).tip
    }

    /// The snapshot of Core §4.3 for one account, and its effective unspent outputs.
    fn coins(
        &self,
        mem: &Mem,
        account: &Account,
        reserved: &Reserved,
    ) -> Result<(balance::Snapshot, Vec<Coin>), PwcError> {
        let chain = mem.chain(&account.info.network);
        let active = chain.active();
        let mut spends: HashMap<OutPoint, usize> = HashMap::new();
        for &i in &active {
            for inp in &chain.txs[i].tx.input {
                if inp.previous_output != OutPoint::null() {
                    *spends.entry(inp.previous_output).or_default() += 1;
                }
            }
        }
        if spends.values().any(|n| *n > 1) {
            return Err(err(
                ErrorCode::StateUnavailable,
                "Unresolved conflict between pending transactions",
            ));
        }
        let confirmed_spent: HashSet<OutPoint> = active
            .iter()
            .filter(|&&i| matches!(chain.txs[i].status, TxStatus::Confirmed { .. }))
            .flat_map(|&i| chain.txs[i].tx.input.iter().map(|inp| inp.previous_output))
            .collect();
        let mut by_key: HashMap<String, (OutPoint, TxOut, usize, TxStatus, bool)> = HashMap::new();
        let mut scenario = Scenario {
            confirmed_outputs: Vec::new(),
            pending_transactions: Vec::new(),
            policy: POLICY,
        };
        for &i in &active {
            let t = &chain.txs[i];
            let txid = t.tx.compute_txid();
            let mut pending_outputs = Vec::new();
            for (vout, out) in t.tx.output.iter().enumerate() {
                let op = OutPoint {
                    txid,
                    vout: vout as u32,
                };
                let owner = mem.owns(account, &out.script_pubkey);
                if let Some(k) = owner {
                    by_key.insert(outpoint_key(&op), (op, out.clone(), k, t.status, t.coinbase));
                }
                match t.status {
                    TxStatus::Confirmed { .. } if owner.is_some() && !confirmed_spent.contains(&op) => {
                        scenario.confirmed_outputs.push(ConfirmedOutput {
                            outpoint: outpoint_key(&op),
                            value: out.value.to_sat(),
                            immature: t.coinbase && chain.confirmations(t.status) < COINBASE_MATURITY,
                            locked: reserved.contains(&op),
                        })
                    }
                    TxStatus::Pending => pending_outputs.push(PendingOutput {
                        outpoint: outpoint_key(&op),
                        value: out.value.to_sat(),
                        owned: owner.is_some(),
                        locked: reserved.contains(&op),
                    }),
                    _ => {}
                }
            }
            if t.status == TxStatus::Pending {
                scenario.pending_transactions.push(PendingTx {
                    txid: txid.to_string(),
                    inputs: t
                        .tx
                        .input
                        .iter()
                        .map(|inp| outpoint_key(&inp.previous_output))
                        .collect(),
                    outputs: pending_outputs,
                });
            }
        }
        let snapshot = balance::compute(&scenario);
        let coins = snapshot
            .outputs
            .iter()
            .map(|e| {
                let (op, txout, key, status, _) = by_key[&e.outpoint].clone();
                let height = match status {
                    TxStatus::Confirmed { height } => Some(height),
                    _ => None,
                };
                Coin {
                    outpoint: op,
                    txout,
                    key,
                    confirmations: chain.confirmations(status),
                    height,
                    class: e.class,
                }
            })
            .collect();
        Ok((snapshot, coins))
    }

    fn address_record(account: &Account, key: &Key, used: bool) -> AddressRecord {
        AddressRecord {
            account_id: account.info.account_id.clone(),
            network: account.info.network.clone(),
            address: key.address.to_string(),
            role: if key.role == Role::Receive { "receive" } else { "change" },
            state: if used { "used" } else { "unused" },
            label: key.label.clone(),
        }
    }

    fn used_scripts(mem: &Mem, network: &str) -> HashSet<ScriptBuf> {
        let chain = mem.chain(network);
        chain
            .active()
            .iter()
            .flat_map(|&i| chain.txs[i].tx.output.iter().map(|o| o.script_pubkey.clone()))
            .collect()
    }
}

impl WalletBackend for MemoryBackend {
    fn owns_script(&self, account_id: &str, script: &bitcoin::Script) -> bool {
        let mem = self.lock();
        mem.owns(mem.account(account_id), script).is_some()
    }

    fn is_unspent(&self, account_id: &str, outpoint: &OutPoint) -> bool {
        let mem = self.lock();
        let account = mem.account(account_id);
        self.coins(&mem, account, &Reserved::new())
            .is_ok_and(|(_, coins)| coins.iter().any(|c| &c.outpoint == outpoint))
    }

    fn fee_estimate(&self, network: &str, _target_blocks: u64) -> Option<u64> {
        self.lock().chain(network).fee_estimate
    }

    fn create_psbt(
        &self,
        account_id: &str,
        request: &CreateRequest,
        excluded: &Reserved,
    ) -> Result<CreatedPsbt, PwcError> {
        if request.fee_rate < MIN_FEE_RATE {
            return Err(err(
                ErrorCode::PolicyViolation,
                "Fee rate below the wallet minimum of 1000 sat/kvB",
            ));
        }
        let mem = self.lock();
        let account = mem.account(account_id);
        let (_, coins) = self.coins(&mem, account, excluded)?;
        let selectable: Vec<&Coin> = coins.iter().filter(|c| c.class == Class::Spendable).collect();
        let candidates: Vec<&Coin> = match &request.selected {
            Some(chosen) => chosen
                .iter()
                .map(|o| selectable.iter().find(|c| &c.outpoint == o).copied())
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| err(ErrorCode::InvalidArgument, "A selected UTXO is not available"))?,
            None => {
                let mut all = selectable.clone();
                all.sort_by_key(|c| std::cmp::Reverse(c.txout.value));
                all
            }
        };
        let change = account
            .keys
            .iter()
            .find(|k| k.role == Role::Change)
            .expect("change key");
        let pay: u64 = request.outputs.iter().map(|(_, v)| v).sum();
        let mut lens: Vec<usize> = request.outputs.iter().map(|(s, _)| s.len()).collect();
        let fee_for = |inputs: usize, lens: &[usize]| {
            crate::types::min_fee(request.fee_rate, crate::psbt::estimate_vsize(inputs, lens))
        };
        let mut chosen: Vec<&Coin> = Vec::new();
        let mut total = 0u64;
        let mut plan = None;
        for coin in &candidates {
            chosen.push(coin);
            total += coin.txout.value.to_sat();
            if request.selected.is_some() && chosen.len() < candidates.len() {
                continue;
            }
            let mut with_change = lens.clone();
            with_change.push(change.script.len());
            let fee_change = fee_for(chosen.len(), &with_change);
            let fee_plain = fee_for(chosen.len(), &lens);
            if total >= pay + fee_change + DUST {
                plan = Some(Some(total - pay - fee_change));
                break;
            }
            if total >= pay + fee_plain {
                plan = Some(None);
                break;
            }
        }
        let Some(change_value) = plan else {
            return Err(err(
                ErrorCode::InsufficientFunds,
                "Insufficient eligible value after fees",
            ));
        };
        let sequence = if request.replaceable {
            Sequence::ENABLE_RBF_NO_LOCKTIME
        } else {
            Sequence::MAX
        };
        let mut outputs: Vec<TxOut> = request
            .outputs
            .iter()
            .map(|(s, v)| TxOut {
                value: Amount::from_sat(*v),
                script_pubkey: s.clone(),
            })
            .collect();
        let mut change_output_indexes = Vec::new();
        if let Some(v) = change_value {
            change_output_indexes.push(outputs.len() as u32);
            outputs.push(TxOut {
                value: Amount::from_sat(v),
                script_pubkey: change.script.clone(),
            });
            lens.push(change.script.len());
        }
        let tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: chosen
                .iter()
                .map(|c| TxIn {
                    previous_output: c.outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence,
                    witness: Witness::new(),
                })
                .collect(),
            output: outputs,
        };
        let fee = total - tx.output.iter().map(|o| o.value.to_sat()).sum::<u64>();
        let mut psbt = bitcoin::Psbt::from_unsigned_tx(tx).expect("unsigned transaction");
        for (input, coin) in psbt.inputs.iter_mut().zip(&chosen) {
            input.witness_utxo = Some(coin.txout.clone());
        }
        Ok(CreatedPsbt {
            psbt,
            fee,
            vsize: crate::psbt::estimate_vsize(chosen.len(), &lens),
            change_output_indexes,
            inputs: chosen.iter().map(|c| c.outpoint).collect(),
        })
    }

    fn owned_output(&self, account_id: &str, outpoint: &OutPoint) -> Option<TxOut> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let chain = mem.chain(&account.info.network);
        let active = chain.active();
        let tx = active
            .iter()
            .map(|&i| &chain.txs[i].tx)
            .find(|t| t.compute_txid() == outpoint.txid)?;
        let out = tx.output.get(outpoint.vout as usize)?;
        mem.owns(account, &out.script_pubkey).map(|_| out.clone())
    }

    fn sign_psbt(&self, account_id: &str, psbt: &mut bitcoin::Psbt, inputs: &[usize]) -> Result<Vec<u32>, PwcError> {
        use bitcoin::secp256k1::Message;
        let mem = self.lock();
        let account = mem.account(account_id);
        let mut signed = Vec::new();
        for &i in inputs {
            let prev = psbt.inputs[i]
                .witness_utxo
                .clone()
                .expect("witness_utxo set for signed inputs");
            let key = &account.keys[mem.owns(account, &prev.script_pubkey).expect("owned input")];
            let public = bitcoin::PublicKey::new(key.public.0);
            if psbt.inputs[i].partial_sigs.contains_key(&public) {
                continue;
            }
            let sighash = bitcoin::sighash::SighashCache::new(&psbt.unsigned_tx)
                .p2wpkh_signature_hash(i, &prev.script_pubkey, prev.value, bitcoin::EcdsaSighashType::All)
                .map_err(|_| err(ErrorCode::PsbtValidationFailed, "Cannot compute the signature hash"))?;
            let signature = self
                .secp
                .sign_ecdsa_low_r(&Message::from_digest(sighash.to_byte_array()), &key.secret);
            let sig = bitcoin::ecdsa::Signature {
                signature,
                sighash_type: bitcoin::EcdsaSighashType::All,
            };
            psbt.inputs[i].partial_sigs.insert(public, sig);
            signed.push(i as u32);
        }
        Ok(signed)
    }

    fn sign_message(
        &self,
        account_id: &str,
        script: &bitcoin::Script,
        message: &str,
        format: MessageFormat,
    ) -> Result<Vec<u8>, PwcError> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let key = &account.keys[mem.owns(account, script).expect("owned address")];
        Ok(match format {
            MessageFormat::Bip322Simple => {
                crate::message::sign_bip322_simple(&self.secp, &key.secret, message.as_bytes())
            }
            MessageFormat::Legacy => crate::message::sign_legacy(&self.secp, &key.secret, message),
            MessageFormat::Bip322Full => {
                return Err(err(ErrorCode::UnsupportedOperation, "bip322-full is not supported"))
            }
        })
    }

    fn broadcast(&self, network: &str, tx: &Transaction) -> Result<Txid, PwcError> {
        let mut mem = self.lock();
        let chain = mem.chain(network);
        let txid = tx.compute_txid();
        let active = chain.active();
        if active.iter().any(|&i| chain.txs[i].tx.compute_txid() == txid) {
            return Ok(txid);
        }
        if chain.offline {
            return Err(err(
                ErrorCode::NetworkUnavailable,
                "The network backend cannot be reached",
            ));
        }
        let rejected = |m: &str| err(ErrorCode::TransactionRejected, m);
        let spent: HashSet<OutPoint> = active
            .iter()
            .flat_map(|&i| chain.txs[i].tx.input.iter().map(|inp| inp.previous_output))
            .collect();
        let mut input_value = 0u64;
        for (index, inp) in tx.input.iter().enumerate() {
            let prev = active
                .iter()
                .map(|&i| &chain.txs[i].tx)
                .find(|t| t.compute_txid() == inp.previous_output.txid)
                .and_then(|t| t.output.get(inp.previous_output.vout as usize))
                .ok_or_else(|| rejected("An input does not exist on this network"))?;
            if spent.contains(&inp.previous_output) {
                return Err(rejected("An input is already spent"));
            }
            if !crate::psbt::verify_input(&self.secp, tx, index, prev) {
                return Err(rejected("An input script does not verify"));
            }
            input_value += prev.value.to_sat();
        }
        if tx.input.is_empty() || tx.output.iter().map(|o| o.value.to_sat()).sum::<u64>() > input_value {
            return Err(rejected("Outputs exceed inputs"));
        }
        Self::add_tx(&mut mem, network, tx.clone(), 0, false);
        Ok(txid)
    }

    fn wallets(&self) -> Vec<WalletInfo> {
        self.lock().wallets.clone()
    }

    fn account(&self, account_id: &str) -> Option<AccountInfo> {
        self.lock().accounts.get(account_id).map(|a| a.info.clone())
    }

    fn balance(&self, account_id: &str, reserved: &Reserved) -> Result<Balance, PwcError> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let (snapshot, _) = self.coins(&mem, account, reserved)?;
        let chain = mem.chain(&account.info.network);
        let wallet = mem
            .wallets
            .iter()
            .find(|w| w.wallet_id == account.info.wallet_id)
            .expect("account's wallet");
        let state = if chain.offline {
            "offline"
        } else if wallet.state == WalletState::Syncing {
            "syncing"
        } else {
            "synced"
        };
        Ok(Balance {
            account_id: account.info.account_id.clone(),
            network: account.info.network.clone(),
            components: snapshot.components,
            updated_at: Timestamp::now(),
            sync: SyncStatus {
                state,
                tip_height: Some(chain.tip),
            },
        })
    }

    fn receive_address(&self, account_id: &str) -> Result<AddressRecord, PwcError> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let used = Self::used_scripts(&mem, &account.info.network);
        let receive: Vec<&Key> = account.keys.iter().filter(|k| k.role == Role::Receive).collect();
        let key = receive
            .iter()
            .find(|k| !used.contains(&k.script))
            .unwrap_or(receive.last().unwrap());
        Ok(Self::address_record(account, key, used.contains(&key.script)))
    }

    fn create_address(&self, account_id: &str, label: Option<&Text>) -> Result<AddressRecord, PwcError> {
        let mut mem = self.lock();
        let account = mem.accounts.get_mut(account_id).expect("existing account");
        let index = account.keys.iter().filter(|k| k.role == Role::Receive).count();
        let mut key = derive_key(
            &self.secp,
            account_id,
            Role::Receive,
            index,
            account.info.script_type,
            &account.info.network,
        );
        key.label = label.cloned();
        let record = Self::address_record(account, &key, false);
        account.keys.push(key);
        Ok(record)
    }

    fn addresses(&self, account_id: &str) -> Vec<AddressRecord> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let used = Self::used_scripts(&mem, &account.info.network);
        account
            .keys
            .iter()
            .map(|k| Self::address_record(account, k, used.contains(&k.script)))
            .collect()
    }

    fn utxos(&self, account_id: &str, reserved: &Reserved) -> Result<Vec<Utxo>, PwcError> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let (_, coins) = self.coins(&mem, account, reserved)?;
        Ok(coins
            .into_iter()
            .map(|c| {
                let key = &account.keys[c.key];
                let restriction = match c.class {
                    Class::Spendable => None,
                    Class::Locked => Some("locked"),
                    Class::Immature => Some("immature"),
                    Class::Unconfirmed => Some("unconfirmed"),
                };
                Utxo {
                    txid: hash256(&c.outpoint.txid),
                    vout: c.outpoint.vout,
                    account_id: account.info.account_id.clone(),
                    network: account.info.network.clone(),
                    value_sats: Sats(c.txout.value.to_sat()),
                    confirmations: c.confirmations,
                    block_height: c.height,
                    address: key.address.to_string(),
                    script_pub_key: Hex(c.txout.script_pubkey.to_bytes()),
                    spendable: restriction.is_none(),
                    restrictions: restriction.into_iter().collect(),
                    is_change: key.role == Role::Change,
                }
            })
            .collect())
    }

    fn transactions(&self, account_id: &str) -> Result<Vec<TransactionRecord>, PwcError> {
        let mem = self.lock();
        let account = mem.account(account_id);
        let chain = mem.chain(&account.info.network);
        let mut records = Vec::new();
        for t in chain.txs.iter().rev() {
            let spent_owned: u64 =
                t.tx.input
                    .iter()
                    .filter_map(|inp| chain.output(&inp.previous_output))
                    .filter(|o| mem.owns(account, &o.script_pubkey).is_some())
                    .map(|o| o.value.to_sat())
                    .sum();
            let received: u64 =
                t.tx.output
                    .iter()
                    .filter(|o| mem.owns(account, &o.script_pubkey).is_some())
                    .map(|o| o.value.to_sat())
                    .sum();
            if spent_owned == 0 && received == 0 {
                continue;
            }
            let all_known =
                t.tx.input
                    .iter()
                    .all(|inp| chain.output(&inp.previous_output).is_some());
            let fee = if t.coinbase || !all_known {
                None
            } else {
                let inputs: u64 =
                    t.tx.input
                        .iter()
                        .map(|inp| chain.output(&inp.previous_output).unwrap().value.to_sat())
                        .sum();
                let outputs: u64 = t.tx.output.iter().map(|o| o.value.to_sat()).sum();
                Some(Sats(inputs - outputs))
            };
            let (status, replaced_by) = match t.status {
                TxStatus::Confirmed { .. } => ("confirmed", None),
                TxStatus::Pending => ("unconfirmed", None),
                TxStatus::Replaced { by } => ("replaced", Some(hash256(&by))),
                TxStatus::Dropped => ("dropped", None),
            };
            let height = match t.status {
                TxStatus::Confirmed { height } => Some(height),
                _ => None,
            };
            records.push(TransactionRecord {
                account_id: account.info.account_id.clone(),
                network: account.info.network.clone(),
                txid: hash256(&t.tx.compute_txid()),
                status,
                confirmations: chain.confirmations(t.status),
                block_height: height,
                first_seen_at: Some(t.first_seen),
                confirmed_at: t.confirmed_at.filter(|_| height.is_some()),
                net_change_sats: SignedSats(received as i64 - spent_owned as i64),
                fee_sats: fee,
                replaced_by_txid: replaced_by,
            });
        }
        Ok(records)
    }
}
