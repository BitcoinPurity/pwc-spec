package org.bitcoinpurity.pwc.provider;

/**
 * One output of a transaction to sign, derived from the PSBT. {@code address} is null when the script has no
 * address form on the account's network; {@code scriptPubKey} is hex.
 */
public record OutputSummary(String address, String scriptPubKey, long amountSats, boolean toThisAccount) {}
