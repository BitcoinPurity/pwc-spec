package org.bitcoinpurity.pwc.provider;

import java.time.Instant;

/** Core §4.3 {@code Balance}. */
public record Balance(
        String accountId, String network, BalanceComponents components, Instant updatedAt, SyncStatus sync) {}
