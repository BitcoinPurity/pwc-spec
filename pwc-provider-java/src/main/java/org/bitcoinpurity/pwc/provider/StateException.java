package org.bitcoinpurity.pwc.provider;

/** A snapshot given to {@link Provider#withState} cannot be loaded. */
public final class StateException extends Exception {
    private static final long serialVersionUID = 1L;

    public enum Kind {
        CORRUPT,
        UNSUPPORTED_VERSION
    }

    private final Kind kind;

    StateException(Kind kind, String message) {
        super(message);
        this.kind = kind;
    }

    public Kind kind() {
        return kind;
    }
}
