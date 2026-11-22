package org.bitcoinpurity.pwc.provider;

import java.util.List;

/** Core §4.5 {@code Utxo}; {@code txid} is lower-case hex, {@code blockHeight} is nullable. */
public record Utxo(
        String txid,
        int vout,
        String accountId,
        String network,
        long valueSats,
        int confirmations,
        Integer blockHeight,
        String address,
        byte[] scriptPubKey,
        boolean spendable,
        List<UtxoRestriction> restrictions,
        boolean isChange) {
    public Utxo {
        restrictions = List.copyOf(restrictions);
    }
}
