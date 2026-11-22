package org.bitcoinpurity.pwc.provider;

import java.util.List;

/** The PSBT (BIP-174 bytes) with the account's signatures added, and the inputs that gained one. */
public record SignedPsbt(byte[] psbt, List<Integer> signedInputs) {
    public SignedPsbt {
        signedInputs = List.copyOf(signedInputs);
    }
}
