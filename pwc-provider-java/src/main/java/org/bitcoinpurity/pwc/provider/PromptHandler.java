package org.bitcoinpurity.pwc.provider;

/**
 * Receives the Provider's requests for a User decision. Called on the thread {@code pwc-prompts}, one prompt
 * at a time; the prompt may be answered later, from any thread. A handler that throws refuses its prompt.
 */
@FunctionalInterface
public interface PromptHandler {
    void onPrompt(Prompt prompt);
}
