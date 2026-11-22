package org.bitcoinpurity.pwc.provider;

import java.nio.file.Path;

/** The LOCAL endpoint could not be opened. */
public final class EndpointException extends Exception {
    private static final long serialVersionUID = 1L;

    public enum Kind {
        UNSAFE_DIRECTORY,
        PATH_TOO_LONG,
        ALREADY_RUNNING,
        IO
    }

    private final Kind kind;
    private final transient Path path;

    EndpointException(Kind kind, String message, String path) {
        super(message);
        this.kind = kind;
        this.path = Path.of(path);
    }

    public Kind kind() {
        return kind;
    }

    public Path path() {
        return path;
    }
}
