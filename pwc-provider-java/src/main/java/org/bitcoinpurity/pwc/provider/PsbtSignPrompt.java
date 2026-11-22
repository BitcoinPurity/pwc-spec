package org.bitcoinpurity.pwc.provider;

public final class PsbtSignPrompt extends Prompt {
    private final PsbtSignRequest request;

    PsbtSignPrompt(long handle, String text, PsbtSignRequest request) {
        super(handle, text);
        this.request = request;
    }

    public PsbtSignRequest request() {
        return request;
    }

    public void approve() {
        answer(true);
    }
}
