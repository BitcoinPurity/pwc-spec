package org.bitcoinpurity.pwc.provider;

import java.time.Instant;

/**
 * Core §4.6 {@code TransactionRecord}; transaction ids are lower-case hex. {@code blockHeight}, {@code firstSeenAt}, {@code confirmedAt},
 * {@code feeSats} and {@code replacedByTxid} are nullable.
 */
public record TransactionRecord(
        String accountId,
        String network,
        String txid,
        TransactionStatus status,
        int confirmations,
        Integer blockHeight,
        Instant firstSeenAt,
        Instant confirmedAt,
        long netChangeSats,
        Long feeSats,
        String replacedByTxid) {}
