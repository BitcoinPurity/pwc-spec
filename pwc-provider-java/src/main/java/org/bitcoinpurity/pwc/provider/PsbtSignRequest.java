package org.bitcoinpurity.pwc.provider;

import java.util.List;

/** A {@code psbt.sign} request; every field is computed from the PSBT and the wallet, not from the Client. */
public record PsbtSignRequest(
        Requester requester,
        String accountId,
        String network,
        List<Integer> inputsToSign,
        int inputCount,
        List<OutputSummary> outputs,
        long feeSats,
        long feeRateSatPerKvb,
        byte[] psbt) {
    public PsbtSignRequest {
        inputsToSign = List.copyOf(inputsToSign);
        outputs = List.copyOf(outputs);
    }
}
