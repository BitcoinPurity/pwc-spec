package org.bitcoinpurity.pwc.provider;

/** A transaction output: an amount in satoshis and its locking script. */
public record TxOut(long valueSats, byte[] scriptPubKey) {}
