package org.bitcoinpurity.pwc.provider;

import java.util.List;
import org.bitcoinj.core.TransactionOutPoint;
import org.bitcoinj.core.TransactionOutput;

/**
 * A payment request for {@code psbt.create}, with addresses already checked against the account's network.
 * {@code feeRate} is in sat/kvB; {@code selected} is null when the wallet chooses the coins.
 */
public record CreateRequest(
        List<TransactionOutput> outputs, long feeRate, List<TransactionOutPoint> selected, boolean replaceable) {
    public CreateRequest {
        outputs = List.copyOf(outputs);
        selected = selected == null ? null : List.copyOf(selected);
    }
}
