package org.bitcoinpurity.pwc.provider;

/** A permission request; the Provider narrows the User's choice to the request (Core §8.1). */
public final class PermissionPrompt extends Prompt {
    private final PermissionRequest request;

    PermissionPrompt(long handle, String text, PermissionRequest request) {
        super(handle, text);
        this.request = request;
    }

    public PermissionRequest request() {
        return request;
    }

    public void grant(GrantChoice choice) {
        answer(choice);
    }
}
