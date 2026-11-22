package org.bitcoinpurity.pwc.provider;

/** The requesting Client as recorded at pairing; its name is a claim. */
public record Requester(String clientId, String claimedName, PeerInfo peer) {}
