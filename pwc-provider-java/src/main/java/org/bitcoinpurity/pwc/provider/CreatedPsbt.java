package org.bitcoinpurity.pwc.provider;

import java.util.List;

/** An unsigned PSBT (BIP-174 bytes) built by the wallet. */
public record CreatedPsbt(
        byte[] psbt, long fee, long vsize, List<Integer> changeOutputIndexes, List<OutPoint> inputs) {
    public CreatedPsbt {
        changeOutputIndexes = List.copyOf(changeOutputIndexes);
        inputs = List.copyOf(inputs);
    }
}
