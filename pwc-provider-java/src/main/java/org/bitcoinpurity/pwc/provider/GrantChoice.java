package org.bitcoinpurity.pwc.provider;

import java.time.Instant;
import java.util.List;

/** The User's decision on a permission request; {@code expiresAt} and {@code constraints} are nullable. */
public record GrantChoice(
        String network,
        List<String> scopes,
        List<String> walletIds,
        List<String> accountIds,
        Instant expiresAt,
        GrantConstraints constraints) {
    public GrantChoice {
        scopes = List.copyOf(scopes);
        walletIds = List.copyOf(walletIds);
        accountIds = List.copyOf(accountIds);
    }
}
