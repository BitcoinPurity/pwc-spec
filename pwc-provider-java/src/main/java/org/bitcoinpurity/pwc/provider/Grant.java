package org.bitcoinpurity.pwc.provider;

import java.time.Instant;
import java.util.List;

/** Core §6 {@code Grant}; {@code expiresAt} and {@code constraints} are nullable. */
public record Grant(
        String grantId,
        String clientId,
        String network,
        List<String> scopes,
        List<String> walletIds,
        List<String> accountIds,
        Instant createdAt,
        Instant expiresAt,
        GrantStatus status,
        GrantConstraints constraints) {
    public Grant {
        scopes = List.copyOf(scopes);
        walletIds = List.copyOf(walletIds);
        accountIds = List.copyOf(accountIds);
    }
}
