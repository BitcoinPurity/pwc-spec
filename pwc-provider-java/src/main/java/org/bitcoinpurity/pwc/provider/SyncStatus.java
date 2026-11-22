package org.bitcoinpurity.pwc.provider;

/** {@code tipHeight} is nullable. */
public record SyncStatus(SyncState state, Integer tipHeight) {}
