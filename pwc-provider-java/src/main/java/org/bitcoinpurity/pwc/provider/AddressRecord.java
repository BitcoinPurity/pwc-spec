package org.bitcoinpurity.pwc.provider;

/** Core §4.4 {@code AddressRecord}; {@code label} is nullable. */
public record AddressRecord(
        String accountId, String network, String address, AddressRole role, AddressState state, String label) {}
