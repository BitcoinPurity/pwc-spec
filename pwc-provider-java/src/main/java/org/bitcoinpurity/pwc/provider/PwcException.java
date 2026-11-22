package org.bitcoinpurity.pwc.provider;

/** Core §10 {@code PwcError}: thrown by the in-memory backend and by Java backends. */
public final class PwcException extends Exception {
    private static final long serialVersionUID = 1L;

    private final ErrorCode code;
    private final String details;
    private final Boolean retryable;

    public PwcException(ErrorCode code, String message) {
        this(code, message, null, null);
    }

    /** {@code details} is JSON text or null; {@code retryable} is nullable. */
    public PwcException(ErrorCode code, String message, String details, Boolean retryable) {
        super(message);
        this.code = code;
        this.details = details;
        this.retryable = retryable;
    }

    public ErrorCode code() {
        return code;
    }

    public String details() {
        return details;
    }

    public Boolean retryable() {
        return retryable;
    }
}
