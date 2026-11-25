package org.bitcoinpurity.pwc.provider;

import java.util.List;

/**
 * A payment request for {@code psbt.create}, with addresses already checked against the account's network.
 * {@code feeRate} is in sat/kvB; {@code selected} is null when the wallet chooses the coins.
 */
public record CreateRequest(
        List<TxOut> outputs, long feeRate, List<OutPoint> selected, boolean replaceable) {
    public CreateRequest {
        outputs = List.copyOf(outputs);
        selected = selected == null ? null : List.copyOf(selected);
    }
}
