package org.bitcoinpurity.pwc.provider;

import java.lang.ref.Reference;
import java.nio.file.Path;
import java.util.Optional;

/** Serves a Provider on a LOCAL endpoint (PWC-TRANSPORT-LOCAL-001) in the background. */
public final class LocalServer implements AutoCloseable {
    /** LOCAL §3.1 provider identifier of the Rust library. */
    public static final String PROVIDER_ID = "pwc-provider";

    private final NativeHandle handle;
    private final Path path;

    private LocalServer(long handle) {
        this.handle = new NativeHandle(this, handle, Native::serverClose);
        this.path = Path.of(Native.serverPath(handle));
    }

    /** The endpoint directory of LOCAL §3.2 for the current user; empty on Windows, whose endpoints are pipes. */
    public static Optional<Path> defaultDirectory() {
        return Optional.ofNullable(Native.serverDefaultDirectory()).map(Path::of);
    }

    /** Serves on the platform's default endpoint for {@link #PROVIDER_ID}. */
    public static LocalServer start(Provider provider) throws EndpointException {
        return start(provider, PROVIDER_ID);
    }

    /**
     * Serves on the platform's default endpoint: {@code <defaultDirectory>/<providerId>.sock} on macOS and Linux,
     * {@code \\.\pipe\pwc-<sid>-<providerId>} on Windows (LOCAL §3).
     */
    public static LocalServer start(Provider provider, String providerId) throws EndpointException {
        return open(provider, null, providerId);
    }

    /** macOS and Linux: listens on {@code <dir>/pwc-provider.sock}. */
    public static LocalServer start(Provider provider, Path dir) throws EndpointException {
        return start(provider, dir, PROVIDER_ID);
    }

    /** macOS and Linux: creates or checks {@code dir} and listens on {@code <dir>/<providerId>.sock}. */
    public static LocalServer start(Provider provider, Path dir, String providerId) throws EndpointException {
        if (System.getProperty("os.name").startsWith("Windows")) {
            throw new UnsupportedOperationException("Windows endpoints are named pipes and have no directory");
        }
        return open(provider, dir.toString(), providerId);
    }

    private static LocalServer open(Provider provider, String dir, String providerId) throws EndpointException {
        try {
            return new LocalServer(Native.serverStart(provider.handle(), dir, providerId));
        } finally {
            Reference.reachabilityFence(provider);
        }
    }

    /** The socket path, or the pipe name on Windows. */
    public Path path() {
        return path;
    }

    /** Stops accepting connections and removes the socket file. Open connections keep running. */
    @Override
    public void close() {
        handle.close();
    }
}
