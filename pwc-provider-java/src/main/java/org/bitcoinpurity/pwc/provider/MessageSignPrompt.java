package org.bitcoinpurity.pwc.provider;

public final class MessageSignPrompt extends Prompt {
    private final MessageSignRequest request;

    MessageSignPrompt(long handle, String text, MessageSignRequest request) {
        super(handle, text);
        this.request = request;
    }

    public MessageSignRequest request() {
        return request;
    }

    public void approve() {
        answer(true);
    }
}
