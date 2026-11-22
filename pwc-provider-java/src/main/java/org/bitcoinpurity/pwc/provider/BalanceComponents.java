package org.bitcoinpurity.pwc.provider;

/** The amounts of Core §4.3, in satoshis; only {@code unconfirmedDeltaSats} may be negative. */
public record BalanceComponents(
        long confirmedSats,
        long pendingSelfSats,
        long pendingExternalSats,
        long pendingOutgoingSats,
        long unconfirmedDeltaSats,
        long spendableSats,
        long lockedSats,
        long immatureSats) {}
