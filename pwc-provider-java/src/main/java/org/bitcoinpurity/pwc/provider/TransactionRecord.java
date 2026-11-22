package org.bitcoinpurity.pwc.provider;

import java.time.Instant;
import org.bitcoinj.base.Sha256Hash;

/**
 * Core §4.6 {@code TransactionRecord}. {@code blockHeight}, {@code firstSeenAt}, {@code confirmedAt},
 * {@code feeSats} and {@code replacedByTxid} are nullable.
 */
public record TransactionRecord(
        String accountId,
        String network,
        Sha256Hash txid,
        TransactionStatus status,
        int confirmations,
        Integer blockHeight,
        Instant firstSeenAt,
        Instant confirmedAt,
        long netChangeSats,
        Long feeSats,
        Sha256Hash replacedByTxid) {}
