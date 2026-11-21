//! PSBT version 0 handling (Core §4.7, §8.3): raw key-value parsing, combination and finalization.

use bitcoin::ecdsa;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Message, Secp256k1, Verification};
use bitcoin::sighash::SighashCache;
use bitcoin::{Psbt, PublicKey, Transaction, TxOut};
use miniscript::psbt::PsbtExt;

use crate::error::{err, ErrorCode, PwcError};

type Map = Vec<(Vec<u8>, Vec<u8>)>;

/// The key-value maps of a PSBT in serialization order.
#[derive(Debug, Clone)]
struct Raw {
    global: Map,
    inputs: Vec<Map>,
    outputs: Vec<Map>,
}

const MAGIC: &[u8] = b"psbt\xff";
const GLOBAL_UNSIGNED_TX: u8 = 0x00;
const GLOBAL_VERSION: u8 = 0xfb;
const IN_PARTIAL_SIG: u8 = 0x02;

fn invalid(m: &str) -> PwcError {
    err(ErrorCode::PsbtValidationFailed, m)
}

fn conflict(m: &str) -> PwcError {
    err(ErrorCode::PsbtConflict, m)
}

fn compact(b: &[u8], pos: &mut usize) -> Option<usize> {
    let first = *b.get(*pos)?;
    *pos += 1;
    let width = match first {
        0xfd => 2,
        0xfe => 4,
        0xff => 8,
        n => return Some(n as usize),
    };
    let bytes = b.get(*pos..*pos + width)?;
    *pos += width;
    let mut v = [0u8; 8];
    v[..width].copy_from_slice(bytes);
    usize::try_from(u64::from_le_bytes(v)).ok()
}

fn read_map(b: &[u8], pos: &mut usize) -> Option<Map> {
    let mut map = Vec::new();
    loop {
        let klen = compact(b, pos)?;
        if klen == 0 {
            return Some(map);
        }
        let key = b.get(*pos..pos.checked_add(klen)?)?.to_vec();
        *pos += klen;
        let vlen = compact(b, pos)?;
        let value = b.get(*pos..pos.checked_add(vlen)?)?.to_vec();
        *pos += vlen;
        map.push((key, value));
    }
}

fn write_compact(out: &mut Vec<u8>, n: usize) {
    match n {
        0..=0xfc => out.push(n as u8),
        0xfd..=0xffff => {
            out.push(0xfd);
            out.extend((n as u16).to_le_bytes());
        }
        0x10000..=0xffff_ffff => {
            out.push(0xfe);
            out.extend((n as u32).to_le_bytes());
        }
        _ => {
            out.push(0xff);
            out.extend((n as u64).to_le_bytes());
        }
    }
}

impl Raw {
    fn serialize(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        for map in std::iter::once(&self.global).chain(&self.inputs).chain(&self.outputs) {
            for (k, v) in map {
                write_compact(&mut out, k.len());
                out.extend(k);
                write_compact(&mut out, v.len());
                out.extend(v);
            }
            out.push(0);
        }
        out
    }

    fn unsigned_tx(&self) -> Option<&[u8]> {
        self.global.iter().find(|(k, _)| k == &[GLOBAL_UNSIGNED_TX]).map(|(_, v)| v.as_slice())
    }
}

/// Rejects PSBTs of a version other than 0 before anything else is parsed.
pub fn check_version(bytes: &[u8]) -> Result<(), PwcError> {
    if !bytes.starts_with(MAGIC) {
        return Err(invalid("Not a PSBT"));
    }
    let mut pos = MAGIC.len();
    let global = read_map(bytes, &mut pos).ok_or_else(|| invalid("Malformed PSBT"))?;
    match global.iter().find(|(k, _)| k == &[GLOBAL_VERSION]) {
        None => Ok(()),
        Some((_, v)) if v.as_slice() == [0, 0, 0, 0] => Ok(()),
        Some(_) => Err(err(ErrorCode::UnsupportedPsbtVersion, "Only PSBT version 0 is supported")),
    }
}

/// Parses a version 0 PSBT into its typed form and its raw maps.
fn parse_raw(bytes: &[u8]) -> Result<(Psbt, Raw), PwcError> {
    let psbt = parse(bytes)?;
    let mut pos = MAGIC.len();
    let global = read_map(bytes, &mut pos).ok_or_else(|| invalid("Malformed PSBT"))?;
    let mut maps = |n: usize| (0..n).map(|_| read_map(bytes, &mut pos)).collect::<Option<Vec<Map>>>();
    let inputs = maps(psbt.inputs.len()).ok_or_else(|| invalid("Malformed PSBT"))?;
    let outputs = maps(psbt.outputs.len()).ok_or_else(|| invalid("Malformed PSBT"))?;
    Ok((psbt, Raw { global, inputs, outputs }))
}

/// Parses a PSBT under BIP-174 after the version check.
pub fn parse(bytes: &[u8]) -> Result<Psbt, PwcError> {
    check_version(bytes)?;
    Psbt::deserialize(bytes).map_err(|e| invalid(&format!("Invalid PSBT: {e}")))
}

/// The previous output of input `index` from the PSBT's own data, checked against the unsigned transaction.
pub fn previous_output(psbt: &Psbt, index: usize) -> Result<Option<TxOut>, PwcError> {
    let input = &psbt.inputs[index];
    let outpoint = psbt.unsigned_tx.input[index].previous_output;
    let from_tx = match &input.non_witness_utxo {
        Some(tx) => {
            if tx.compute_txid() != outpoint.txid {
                return Err(invalid("non_witness_utxo does not match the input"));
            }
            Some(tx.output.get(outpoint.vout as usize).cloned().ok_or_else(|| invalid("Previous output index out of range"))?)
        }
        None => None,
    };
    match (&input.witness_utxo, from_tx) {
        (Some(w), Some(t)) if *w != t => Err(invalid("witness_utxo and non_witness_utxo disagree")),
        (Some(w), _) => Ok(Some(w.clone())),
        (None, t) => Ok(t),
    }
}

/// Verifies one ECDSA partial signature of input `index` against the PSBT's previous-output data.
fn verify_partial_sig<C: Verification>(secp: &Secp256k1<C>, psbt: &Psbt, index: usize, key: &[u8], sig: &[u8]) -> bool {
    let (Ok(pk), Ok(sig)) = (PublicKey::from_slice(key), ecdsa::Signature::from_slice(sig)) else { return false };
    let Ok(Some(prev)) = previous_output(psbt, index) else { return false };
    let input = &psbt.inputs[index];
    let mut cache = SighashCache::new(&psbt.unsigned_tx);
    let spk = &prev.script_pubkey;
    let segwit_script = if spk.is_p2wpkh() || spk.is_p2wsh() {
        Some(spk.clone())
    } else if spk.is_p2sh() {
        input.redeem_script.clone().filter(|r| r.is_p2wpkh() || r.is_p2wsh())
    } else {
        None
    };
    let digest = match segwit_script {
        Some(s) if s.is_p2wpkh() => cache.p2wpkh_signature_hash(index, &s, prev.value, sig.sighash_type).ok().map(|h| h.to_byte_array()),
        Some(_) => input
            .witness_script
            .as_ref()
            .and_then(|ws| cache.p2wsh_signature_hash(index, ws, prev.value, sig.sighash_type).ok())
            .map(|h| h.to_byte_array()),
        None => {
            let code = if spk.is_p2sh() { input.redeem_script.clone() } else { Some(spk.clone()) };
            code.and_then(|c| cache.legacy_signature_hash(index, &c, sig.sighash_type.to_u32()).ok()).map(|h| h.to_byte_array())
        }
    };
    match digest {
        Some(d) => secp.verify_ecdsa(&Message::from_digest(d), &sig.signature, &pk.inner).is_ok(),
        None => false,
    }
}

fn merge(into: &mut Map, from: &Map, mut resolve: impl FnMut(&[u8], &[u8], &[u8]) -> Result<(), PwcError>) -> Result<(), PwcError> {
    for (k, v) in from {
        match into.iter().find(|(ik, _)| ik == k) {
            None => into.push((k.clone(), v.clone())),
            Some((_, iv)) if iv == v => {}
            Some((_, iv)) => resolve(k, iv, v)?,
        }
    }
    Ok(())
}

/// Core §4.7 `psbt.combine`.
pub fn combine(psbts: &[Vec<u8>]) -> Result<Vec<u8>, PwcError> {
    psbts.iter().try_for_each(|b| check_version(b))?;
    let parsed: Vec<(Psbt, Raw)> = psbts.iter().map(|b| parse_raw(b)).collect::<Result<_, _>>()?;
    let first_tx = parsed[0].1.unsigned_tx();
    if parsed.iter().any(|(_, r)| r.unsigned_tx() != first_tx) {
        return Err(conflict("The PSBTs are not for the same transaction"));
    }
    let secp = Secp256k1::verification_only();
    let mut merged = parsed[0].1.clone();
    for (psbt, raw) in &parsed[1..] {
        merge(&mut merged.global, &raw.global, |_, _, _| Err(conflict("Conflicting global values")))?;
        for (i, map) in raw.inputs.iter().enumerate() {
            merge(&mut merged.inputs[i], map, |k, kept, new| {
                let both_verify = k[0] == IN_PARTIAL_SIG
                    && verify_partial_sig(&secp, &parsed[0].0, i, &k[1..], kept)
                    && verify_partial_sig(&secp, psbt, i, &k[1..], new);
                if both_verify {
                    Ok(())
                } else {
                    Err(conflict("Conflicting values for the same input key"))
                }
            })?;
        }
        for (i, map) in raw.outputs.iter().enumerate() {
            merge(&mut merged.outputs[i], map, |_, _, _| Err(conflict("Conflicting values for the same output key")))?;
        }
    }
    let bytes = merged.serialize();
    Psbt::deserialize(&bytes).map_err(|e| invalid(&format!("Combined PSBT is invalid: {e}")))?;
    Ok(bytes)
}

pub struct Finalized {
    pub psbt: Vec<u8>,
    pub transaction: Option<Transaction>,
}

/// Core §4.7 `psbt.finalize`: finalizes what existing signatures allow and extracts the transaction once every
/// input is final and verified.
pub fn finalize(bytes: &[u8]) -> Result<Finalized, PwcError> {
    let mut psbt = parse(bytes)?;
    for i in 0..psbt.inputs.len() {
        if previous_output(&psbt, i)?.is_none() {
            return Err(invalid("An input lacks previous-output data"));
        }
    }
    let secp = Secp256k1::verification_only();
    let mut complete = true;
    for i in 0..psbt.inputs.len() {
        let input = &psbt.inputs[i];
        if input.final_script_sig.is_some() || input.final_script_witness.is_some() {
            continue;
        }
        let mut attempt = psbt.clone();
        if attempt.finalize_inp_mut(&secp, i).is_ok() {
            psbt = attempt;
        } else {
            complete = false;
        }
    }
    if !complete {
        return Ok(Finalized { psbt: psbt.serialize(), transaction: None });
    }
    let tx = psbt.extract(&secp).map_err(|e| invalid(&format!("Final scripts do not verify: {e}")))?;
    Ok(Finalized { psbt: psbt.serialize(), transaction: Some(tx) })
}

fn compact_len(n: usize) -> u64 {
    match n {
        0..=0xfc => 1,
        0xfd..=0xffff => 3,
        _ => 5,
    }
}

/// BIP-141 virtual size of a transaction with `inputs` P2WPKH inputs once signed, and outputs with the given
/// script lengths. A P2WPKH witness is at most 108 weight units with low-R signatures.
pub fn estimate_vsize(inputs: usize, output_script_lens: &[usize]) -> u64 {
    let outputs: u64 = output_script_lens.iter().map(|&l| 8 + compact_len(l) + l as u64).sum();
    let base = 4 + compact_len(inputs) + 41 * inputs as u64 + compact_len(output_script_lens.len()) + outputs + 4;
    let weight = 4 * base + 2 + 108 * inputs as u64;
    weight.div_ceil(4)
}

/// Verifies that input `index` of `tx` validly spends `prevout`, for P2WPKH and P2PKH outputs only.
pub fn verify_input<C: Verification>(secp: &Secp256k1<C>, tx: &Transaction, index: usize, prevout: &TxOut) -> bool {
    let spk = &prevout.script_pubkey;
    let txin = &tx.input[index];
    let mut cache = SighashCache::new(tx);
    let (sig, pk, digest) = if spk.is_p2wpkh() {
        if !txin.script_sig.is_empty() || txin.witness.len() != 2 {
            return false;
        }
        let (Ok(sig), Ok(pk)) = (ecdsa::Signature::from_slice(&txin.witness[0]), bitcoin::CompressedPublicKey::from_slice(&txin.witness[1]))
        else {
            return false;
        };
        if bitcoin::ScriptBuf::new_p2wpkh(&pk.wpubkey_hash()) != *spk {
            return false;
        }
        let Ok(h) = cache.p2wpkh_signature_hash(index, spk, prevout.value, sig.sighash_type) else { return false };
        (sig, pk.0, h.to_byte_array())
    } else if spk.is_p2pkh() {
        let pushes: Vec<Vec<u8>> = txin
            .script_sig
            .instructions()
            .filter_map(|i| match i {
                Ok(bitcoin::script::Instruction::PushBytes(b)) => Some(b.as_bytes().to_vec()),
                _ => None,
            })
            .collect();
        if pushes.len() != 2 || txin.script_sig.instructions().count() != 2 || !txin.witness.is_empty() {
            return false;
        }
        let (Ok(sig), Ok(pk)) = (ecdsa::Signature::from_slice(&pushes[0]), PublicKey::from_slice(&pushes[1])) else {
            return false;
        };
        if bitcoin::ScriptBuf::new_p2pkh(&pk.pubkey_hash()) != *spk {
            return false;
        }
        let Ok(h) = cache.legacy_signature_hash(index, spk, sig.sighash_type.to_u32()) else { return false };
        (sig, pk.inner, h.to_byte_array())
    } else {
        return false;
    };
    secp.verify_ecdsa(&Message::from_digest(digest), &sig.signature, &pk).is_ok()
}

/// Core §4.7 `signing_status` for P2WPKH and P2PKH inputs; other scripts make completeness unknown.
pub fn signing_status(psbt: &Psbt) -> &'static str {
    let mut unknown = false;
    for (i, input) in psbt.inputs.iter().enumerate() {
        if input.final_script_sig.is_some() || input.final_script_witness.is_some() {
            continue;
        }
        let Ok(Some(prev)) = previous_output(psbt, i) else {
            unknown = true;
            continue;
        };
        let spk = &prev.script_pubkey;
        let signed_by = |f: &dyn Fn(&PublicKey) -> bitcoin::ScriptBuf| input.partial_sigs.keys().any(|pk| f(pk) == *spk);
        let satisfied = if spk.is_p2wpkh() {
            signed_by(&|pk| pk.wpubkey_hash().map(|h| bitcoin::ScriptBuf::new_p2wpkh(&h)).unwrap_or_default())
        } else if spk.is_p2pkh() {
            signed_by(&|pk| bitcoin::ScriptBuf::new_p2pkh(&pk.pubkey_hash()))
        } else {
            unknown = true;
            continue;
        };
        if !satisfied {
            return "partial";
        }
    }
    if unknown {
        "unknown"
    } else {
        "complete"
    }
}

/// Estimated virtual size of the transaction once signed, from the previous output of each input.
pub fn estimate_signed_vsize(psbt: &Psbt, prevouts: &[Option<TxOut>]) -> u64 {
    let base = bitcoin::consensus::serialize(&psbt.unsigned_tx).len() as u64;
    let extra: u64 = prevouts
        .iter()
        .map(|p| match p {
            Some(o) if o.script_pubkey.is_p2wpkh() => 108,
            Some(o) if o.script_pubkey.is_p2pkh() => 4 * 107,
            _ => 0,
        })
        .sum();
    (4 * base + 2 + extra).div_ceil(4)
}
