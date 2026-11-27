package org.bitcoinpurity.pwc.provider;

import java.lang.ref.Reference;
import java.util.List;

/**
 * A PWC Provider: Core v0.2.1 semantics, state and approval flow, implemented by the Rust library. Serve it
 * with {@link LocalServer}.
 */
public final class Provider implements AutoCloseable {
    private final NativeHandle handle;
    private final Config config;

    private Provider(long handle, Config config) {
        this.handle = new NativeHandle(this, handle, Native::providerFree);
        this.config = config;
    }

    public static Provider create(WalletBackend backend, Config config, PromptHandler prompts) {
        try {
            return new Provider(Native.providerCreate(memory(backend), backend, config, new Dispatcher(prompts)), config);
        } finally {
            Reference.reachabilityFence(backend);
        }
    }

    /**
     * Restores a Provider from a snapshot made by {@link #saveState()}. Operations that were waiting for the User
     * fail with a retryable {@code INTERNAL_ERROR}.
     */
    public static Provider withState(WalletBackend backend, Config config, PromptHandler prompts, byte[] snapshot)
            throws StateException {
        try {
            return new Provider(
                    Native.providerRestore(memory(backend), backend, config, new Dispatcher(prompts), snapshot), config);
        } finally {
            Reference.reachabilityFence(backend);
        }
    }

    private static long memory(WalletBackend backend) {
        return backend instanceof MemoryBackend m ? m.handle() : 0;
    }

    /**
     * The paired Clients and their secrets, grants, Operations, idempotency records and reservations. It contains
     * secrets: store it at least as safely as the wallet files (LOCAL §7).
     */
    public byte[] saveState() {
        try {
            return Native.providerSaveState(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    public Config config() {
        return config;
    }

    public List<PairedClient> pairedClients() {
        try {
            return List.copyOf(Native.providerPairedClients(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Every grant, for the wallet's grant review interface (Core §6). */
    public List<Grant> grants() {
        try {
            return List.copyOf(Native.providerGrants(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Revokes a grant from the wallet's own interface; false if there is no such grant. */
    public boolean revokeGrant(String grantId) {
        try {
            return Native.providerRevokeGrant(handle(), grantId);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Unpairs a Client from the wallet's own interface; same effect as {@code local.unpair} (LOCAL §5.7). */
    public void unpairClient(String clientId) {
        try {
            Native.providerUnpairClient(handle(), clientId);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Reports a wallet change; authorized Clients receive the corresponding event. The event is queued and delivered
     * by a Provider thread, so this returns at once and may be called from any thread, including from inside a
     * {@link WalletBackend} method.
     */
    public void notify(BackendEvent event) {
        try {
            Native.providerNotify(handle(), event);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    long handle() {
        return handle.get();
    }

    /** Releases this handle. Servers and their open connections keep the Provider running until they end. */
    @Override
    public void close() {
        handle.close();
    }

    /** Called by the native prompt thread. */
    static final class Dispatcher {
        private final PromptHandler handler;

        Dispatcher(PromptHandler handler) {
            this.handler = handler;
        }

        void deliver(Prompt prompt) {
            try {
                handler.onPrompt(prompt);
            } catch (RuntimeException | Error e) {
                try {
                    prompt.reject();
                } catch (IllegalStateException answered) {
                    // The handler answered before it threw.
                }
                Thread t = Thread.currentThread();
                t.getUncaughtExceptionHandler().uncaughtException(t, e);
            }
        }
    }
}
