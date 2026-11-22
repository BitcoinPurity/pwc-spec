package org.bitcoinpurity.pwc.provider;

/** A change in the wallet that the host reports with {@link Provider#notify(BackendEvent)} (Core §8.6). */
public sealed interface BackendEvent {
    record WalletLocked(String walletId) implements BackendEvent {}

    record WalletUnlocked(String walletId) implements BackendEvent {}

    record WalletSyncChanged(String walletId) implements BackendEvent {}

    record BalanceChanged(String accountId) implements BackendEvent {}

    record TransactionChanged(String accountId, String txid, TransactionStatus status) implements BackendEvent {}
}
