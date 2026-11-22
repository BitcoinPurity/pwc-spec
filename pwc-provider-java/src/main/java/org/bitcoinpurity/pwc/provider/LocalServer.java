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

    /** The endpoint directory of LOCAL §3.2 for the current user. */
    public static Optional<Path> defaultDirectory() {
        return Optional.ofNullable(Native.serverDefaultDirectory()).map(Path::of);
    }

    public static LocalServer start(Provider provider, Path dir) throws EndpointException {
        return start(provider, dir, PROVIDER_ID);
    }

    /** Creates or checks {@code dir} and listens on {@code <dir>/<providerId>.sock}. */
    public static LocalServer start(Provider provider, Path dir, String providerId) throws EndpointException {
        try {
            return new LocalServer(Native.serverStart(provider.handle(), dir.toString(), providerId));
        } finally {
            Reference.reachabilityFence(provider);
        }
    }

    public Path path() {
        return path;
    }

    /** Stops accepting connections and removes the socket file. Open connections keep running. */
    @Override
    public void close() {
        handle.close();
    }
}
