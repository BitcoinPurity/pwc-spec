package org.bitcoinpurity.pwc.provider;

/** A {@code message.sign} request with the exact message. */
public record MessageSignRequest(
        Requester requester, String accountId, String network, String address, String message, MessageFormat format) {}
