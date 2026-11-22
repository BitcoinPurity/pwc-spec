package org.bitcoinpurity.pwc.provider;

import java.util.List;

/** A wallet with all of its accounts, before any grant filtering. */
public record WalletInfo(
        String walletId, String displayName, String network, WalletKind kind, WalletState state, List<String> accountIds) {
    public WalletInfo {
        accountIds = List.copyOf(accountIds);
    }
}
