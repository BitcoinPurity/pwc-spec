package org.bitcoinpurity.pwc.provider;

import java.time.Instant;

/** Core §6 grant constraints; either field is null when absent. */
public record GrantConstraints(Long maxAmountSatsPerRequest, Instant validUntil) {}
