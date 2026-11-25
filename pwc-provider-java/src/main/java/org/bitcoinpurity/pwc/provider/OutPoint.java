package org.bitcoinpurity.pwc.provider;

/** A transaction output reference; {@code txid} is lower-case hex in display order. */
public record OutPoint(String txid, int vout) {}
