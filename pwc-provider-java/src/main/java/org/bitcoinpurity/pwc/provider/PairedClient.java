package org.bitcoinpurity.pwc.provider;

import java.time.Instant;

/** A paired Client (LOCAL §5.7); {@code claimedDescription} is nullable. */
public record PairedClient(
        String clientId,
        String claimedName,
        String claimedDescription,
        Instant pairedAt,
        Instant lastConnectedAt,
        PeerInfo peer) {}
