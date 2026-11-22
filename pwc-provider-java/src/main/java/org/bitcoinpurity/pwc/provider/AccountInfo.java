package org.bitcoinpurity.pwc.provider;

import java.util.List;

/**
 * An account. {@code methods} are the account-scoped methods usable with it (Core §4.2); names that are not
 * Core methods are ignored.
 */
public record AccountInfo(
        String accountId,
        String walletId,
        String displayName,
        String network,
        boolean canSign,
        ScriptType scriptType,
        List<String> methods) {
    public AccountInfo {
        methods = List.copyOf(methods);
    }
}
