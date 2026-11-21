//! Message signatures (Core §8.4): BIP-322 simple for P2WPKH and the legacy compact format for P2PKH.

use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::secp256k1::{ecdsa::RecoverableSignature, Message, Secp256k1, SecretKey, Signing, Verification};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::sign_message::{signed_msg_hash, MessageSignature};
use bitcoin::{
    absolute, transaction, Address, Amount, OutPoint, Script, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};

/// The BIP-322 tagged hash of a message.
pub fn bip322_hash(message: &[u8]) -> [u8; 32] {
    let tag = sha256::Hash::hash(b"BIP0322-signed-message");
    let mut e = sha256::Hash::engine();
    e.input(tag.as_byte_array());
    e.input(tag.as_byte_array());
    e.input(message);
    sha256::Hash::from_engine(e).to_byte_array()
}

fn to_spend(script_pubkey: &Script, message: &[u8]) -> Transaction {
    let script_sig = bitcoin::script::Builder::new()
        .push_opcode(bitcoin::opcodes::OP_0)
        .push_slice(bip322_hash(message))
        .into_script();
    Transaction {
        version: transaction::Version(0),
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0xffff_ffff,
            },
            script_sig,
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::ZERO,
            script_pubkey: script_pubkey.to_owned(),
        }],
    }
}

fn to_sign(script_pubkey: &Script, message: &[u8], witness: Witness) -> Transaction {
    Transaction {
        version: transaction::Version(0),
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: to_spend(script_pubkey, message).compute_txid(),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ZERO,
            witness,
        }],
        output: vec![TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::from_bytes(vec![0x6a]),
        }],
    }
}

/// A BIP-322 simple signature by the P2WPKH key `secret`: the consensus-encoded witness of `to_sign`.
pub fn sign_bip322_simple<C: Signing>(secp: &Secp256k1<C>, secret: &SecretKey, message: &[u8]) -> Vec<u8> {
    let public = bitcoin::CompressedPublicKey(secret.public_key(secp));
    let spk = ScriptBuf::new_p2wpkh(&public.wpubkey_hash());
    let tx = to_sign(&spk, message, Witness::new());
    let sighash = SighashCache::new(&tx)
        .p2wpkh_signature_hash(0, &spk, Amount::ZERO, EcdsaSighashType::All)
        .expect("P2WPKH script and input 0");
    let signature = secp.sign_ecdsa_low_r(&Message::from_digest(sighash.to_byte_array()), secret);
    let sig = bitcoin::ecdsa::Signature {
        signature,
        sighash_type: EcdsaSighashType::All,
    };
    bitcoin::consensus::serialize(&Witness::p2wpkh(&sig, &public.0))
}

/// `None` if the signature is not a well-formed witness; otherwise whether it verifies.
pub fn verify_bip322_simple<C: Verification>(
    secp: &Secp256k1<C>,
    address: &Address,
    message: &[u8],
    signature: &[u8],
) -> Option<bool> {
    let witness: Witness = bitcoin::consensus::deserialize(signature).ok()?;
    let spk = address.script_pubkey();
    let tx = to_sign(&spk, message, witness);
    Some(crate::psbt::verify_input(
        secp,
        &tx,
        0,
        &TxOut {
            value: Amount::ZERO,
            script_pubkey: spk,
        },
    ))
}

/// The 65-byte compact signature with the network's signed-message prefix.
pub fn sign_legacy<C: Signing>(secp: &Secp256k1<C>, secret: &SecretKey, message: &str) -> Vec<u8> {
    let hash = signed_msg_hash(message);
    let sig: RecoverableSignature = secp.sign_ecdsa_recoverable(&Message::from_digest(hash.to_byte_array()), secret);
    MessageSignature::new(sig, true).serialize().to_vec()
}

/// `None` if the signature is malformed; otherwise whether it was made by the key of the P2PKH `address`.
pub fn verify_legacy<C: Verification>(
    secp: &Secp256k1<C>,
    address: &Address,
    message: &str,
    signature: &[u8],
) -> Option<bool> {
    let sig = MessageSignature::from_slice(signature).ok()?;
    sig.is_signed_by_address(secp, address, signed_msg_hash(message)).ok()
}
