package org.bitcoinpurity.pwc.provider;

/** LOCAL §5.3 pairing prompt; the name and description are claims. {@code claimedDescription} is nullable. */
public record PairingRequest(String claimedName, String claimedDescription, String pairingCode, PeerInfo peer) {}
