package org.bitcoinpurity.pwc.provider;

import java.lang.ref.Reference;

/**
 * A request for a User decision. A prompt that is never answered is refused when it is garbage-collected. An
 * answer to a withdrawn prompt has no effect; a second answer throws {@link IllegalStateException}.
 */
public abstract sealed class Prompt permits PairingPrompt, PermissionPrompt, PsbtSignPrompt, MessageSignPrompt {
    private final NativeHandle handle;
    private final String text;

    Prompt(long handle, String text) {
        this.handle = new NativeHandle(this, handle, Native::promptFree);
        this.text = text;
    }

    /** Whether the Provider withdrew the prompt: its connection closed, it expired or was canceled. */
    public boolean isWithdrawn() {
        try {
            return Native.promptIsWithdrawn(handle.get());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Refuses the request. */
    public void reject() {
        answer(false);
    }

    final void answer(boolean approve) {
        boolean first;
        try {
            first = Native.promptAnswer(handle.get(), approve);
        } finally {
            Reference.reachabilityFence(this);
        }
        if (!first) {
            throw new IllegalStateException("the prompt was already answered");
        }
    }

    final void answer(GrantChoice choice) {
        boolean first;
        try {
            first = Native.promptGrant(handle.get(), choice);
        } finally {
            Reference.reachabilityFence(this);
        }
        if (!first) {
            throw new IllegalStateException("the prompt was already answered");
        }
    }

    /** The text the Rust library shows for this prompt. */
    @Override
    public String toString() {
        return text;
    }
}
