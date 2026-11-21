//! PWC-TRANSPORT-LOCAL-001 v0.1.0.

use bitcoin::hashes::{sha256, Hash, HashEngine, Hmac, HmacEngine};

fn proof(key: &[u8; 32], label: &[u8], first: &[u8; 32], second: &[u8; 32], client_id: &str) -> [u8; 32] {
    let mut engine = HmacEngine::<sha256::Hash>::new(key);
    engine.input(label);
    engine.input(&[0]);
    engine.input(first);
    engine.input(second);
    engine.input(client_id.as_bytes());
    Hmac::<sha256::Hash>::from_engine(engine).to_byte_array()
}

/// LOCAL §5.4 `client_proof`.
pub fn client_proof(secret: &[u8; 32], server_nonce: &[u8; 32], client_nonce: &[u8; 32], client_id: &str) -> [u8; 32] {
    proof(secret, b"PWC-LOCAL-1 client", server_nonce, client_nonce, client_id)
}

/// LOCAL §5.4 `server_proof`.
pub fn server_proof(secret: &[u8; 32], server_nonce: &[u8; 32], client_nonce: &[u8; 32], client_id: &str) -> [u8; 32] {
    proof(secret, b"PWC-LOCAL-1 provider", client_nonce, server_nonce, client_id)
}
