package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.fasterxml.jackson.databind.JsonNode;
import org.bitcoinj.base.Sha256Hash;
import org.junit.jupiter.api.Test;

class EventsTest {
    @Test
    void notifyDeliversEventsToAuthorizedClients() throws Exception {
        try (Harness h = Harness.start()) {
            LocalClient listener = h.client("events:read", "wallets:read", "balances:read", "transactions:read").client();
            LocalClient noEvents = h.client("balances:read").client();

            h.provider.notify(new BackendEvent.BalanceChanged("a-regtest-wpkh"));
            JsonNode e = listener.nextEvent();
            assertEquals("account.balanceChanged", e.get("event_type").asText());
            assertEquals("a-regtest-wpkh", e.get("account_id").asText());
            noEvents.noEvent();

            Sha256Hash txid = Sha256Hash.wrap("ab".repeat(32));
            h.provider.notify(new BackendEvent.TransactionChanged("a-regtest-wpkh", txid, TransactionStatus.CONFIRMED));
            e = listener.nextEvent();
            assertEquals("account.transactionChanged", e.get("event_type").asText());
            assertEquals(txid.toString(), e.get("payload").get("txid").asText());
            assertEquals("confirmed", e.get("payload").get("status").asText());

            h.provider.notify(new BackendEvent.WalletSyncChanged("w-regtest-main"));
            e = listener.nextEvent();
            assertEquals("wallet.syncChanged", e.get("event_type").asText());
            assertEquals("synced", e.get("payload").get("sync").get("state").asText());

            h.memory.setWalletState("w-regtest-main", WalletState.LOCKED);
            h.provider.notify(new BackendEvent.WalletLocked("w-regtest-main"));
            e = listener.nextEvent();
            assertEquals("wallet.locked", e.get("event_type").asText());
            assertEquals("w-regtest-main", e.get("payload").get("wallet_id").asText());
            h.provider.notify(new BackendEvent.BalanceChanged("a-regtest-wpkh"));
            listener.noEvent();

            h.memory.setWalletState("w-regtest-main", WalletState.READY);
            h.provider.notify(new BackendEvent.WalletUnlocked("w-regtest-main"));
            assertEquals("wallet.unlocked", listener.nextEvent().get("event_type").asText());

            h.provider.notify(new BackendEvent.BalanceChanged("a-mainnet-wpkh"));
            listener.noEvent();
        }
    }
}
