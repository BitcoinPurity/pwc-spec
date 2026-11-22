package org.bitcoinpurity.pwc.provider;

import java.util.List;
import org.bitcoinj.base.Sha256Hash;
import org.bitcoinj.script.Script;

/** Core §4.5 {@code Utxo}; {@code blockHeight} is nullable. */
public record Utxo(
        Sha256Hash txid,
        int vout,
        String accountId,
        String network,
        long valueSats,
        int confirmations,
        Integer blockHeight,
        String address,
        Script scriptPubKey,
        boolean spendable,
        List<UtxoRestriction> restrictions,
        boolean isChange) {
    public Utxo {
        restrictions = List.copyOf(restrictions);
    }
}
