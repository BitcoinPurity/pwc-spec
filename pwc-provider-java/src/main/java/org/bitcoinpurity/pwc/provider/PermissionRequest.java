package org.bitcoinpurity.pwc.provider;

import java.time.Instant;
import java.util.List;

/**
 * Core §8.1 permission request; unknown wallet and account identifiers have been removed. {@code network},
 * {@code expiresAt} and {@code constraints} are nullable.
 */
public record PermissionRequest(
        Requester requester,
        List<String> scopes,
        String network,
        List<String> walletIds,
        List<String> accountIds,
        Instant expiresAt,
        GrantConstraints constraints) {
    public PermissionRequest {
        scopes = List.copyOf(scopes);
        walletIds = List.copyOf(walletIds);
        accountIds = List.copyOf(accountIds);
    }
}
