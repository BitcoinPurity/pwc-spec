package org.bitcoinpurity.pwc.provider;

public final class PairingPrompt extends Prompt {
    private final PairingRequest request;

    PairingPrompt(long handle, String text, PairingRequest request) {
        super(handle, text);
        this.request = request;
    }

    public PairingRequest request() {
        return request;
    }

    public void approve() {
        answer(true);
    }
}
