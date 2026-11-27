package org.bitcoinpurity.pwc.provider;

import static org.bitcoinpurity.pwc.provider.LocalClient.obj;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTimeoutPreemptively;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.junit.jupiter.api.Test;

class JavaBackendTest {
    static final String WPKH = "a-regtest-wpkh";
    static final String[] READ_SCOPES = {
        "wallets:read", "balances:read", "addresses:read", "addresses:create", "transactions:read", "utxos:read"
    };

    /** Every read method, with timestamps removed from the answers. */
    static List<JsonNode> readEverything(Harness h) throws Exception {
        LocalClient c = h.client(READ_SCOPES).client();
        List<JsonNode> out = new ArrayList<>();
        out.add(c.ok("wallet.list", obj()));
        out.add(c.ok("account.list", obj("wallet_id", "w-regtest-main")));
        out.add(c.ok("account.getBalance", obj("account_id", WPKH)));
        out.add(c.ok("address.getReceiveAddress", obj("account_id", WPKH)));
        out.add(c.ok("address.create", obj("account_id", WPKH, "label", "Invoice 7", "idempotency_key", "a-1")));
        out.add(c.ok("address.list", obj("account_id", WPKH)));
        JsonNode utxos = c.ok("utxo.list", obj("account_id", WPKH));
        out.add(utxos);
        out.add(c.ok("transaction.list", obj("account_id", WPKH)));
        String txid = utxos.get("utxos").get(0).get("txid").asText();
        out.add(c.ok("transaction.get", obj("account_id", WPKH, "txid", txid)));
        out.forEach(JavaBackendTest::dropTimes);
        return out;
    }

    static void dropTimes(JsonNode n) {
        if (n instanceof ObjectNode o) {
            for (Iterator<Map.Entry<String, JsonNode>> it = o.properties().iterator(); it.hasNext(); ) {
                Map.Entry<String, JsonNode> e = it.next();
                if (e.getKey().endsWith("_at")) {
                    it.remove();
                } else {
                    dropTimes(e.getValue());
                }
            }
        } else {
            n.forEach(JavaBackendTest::dropTimes);
        }
    }

    @Test
    void readMethodsAnswerAsWithTheInMemoryBackend() throws Exception {
        List<JsonNode> direct;
        try (Harness h = Harness.start()) {
            direct = readEverything(h);
        }
        ForwardingBackend[] java = new ForwardingBackend[1];
        try (Harness h = Harness.start(Config.defaults(), m -> java[0] = new ForwardingBackend(m))) {
            assertEquals(direct, readEverything(h));
        }
        assertTrue(java[0].called.containsAll(Set.of(
                "wallets", "account", "balance", "receiveAddress", "createAddress", "addresses", "utxos",
                "transactions")), java[0].called.toString());
    }

    @Test
    void pwcExceptionBecomesTheRequestsError() throws Exception {
        ForwardingBackend[] java = new ForwardingBackend[1];
        try (Harness h = Harness.start(Config.defaults(), m -> java[0] = new ForwardingBackend(m))) {
            LocalClient c = h.client(READ_SCOPES).client();
            java[0].failures.put("balance", new PwcException(
                    ErrorCode.SYNC_INCOMPLETE, "Still syncing", "{\"progress\":0.5}", true));
            JsonNode r = c.call("account.getBalance", obj("account_id", WPKH));
            JsonNode e = r.get("error").get("data");
            assertEquals("SYNC_INCOMPLETE", e.get("code").asText());
            assertEquals("Still syncing", e.get("message").asText());
            assertEquals(0.5, e.get("details").get("progress").asDouble());
            assertTrue(e.get("retryable").asBoolean());
        }
    }

    @Test
    void notifyFromInsideABackendMethod() throws Exception {
        ForwardingBackend[] java = new ForwardingBackend[1];
        try (Harness h = Harness.start(Config.defaults(), m -> java[0] = new ForwardingBackend(m))) {
            LocalClient c = h.client("balances:read", "events:read").client();
            java[0].onBalance = () -> h.provider.notify(new BackendEvent.BalanceChanged(WPKH));
            assertTimeoutPreemptively(Duration.ofSeconds(10), () -> {
                c.ok("account.getBalance", obj("account_id", WPKH));
                assertEquals("account.balanceChanged", c.nextEvent().get("event_type").asText());
            });
        }
    }

    @Test
    void otherExceptionsGiveInternalError() throws Exception {
        ForwardingBackend[] java = new ForwardingBackend[1];
        try (Harness h = Harness.start(Config.defaults(), m -> java[0] = new ForwardingBackend(m))) {
            LocalClient c = h.client(READ_SCOPES).client();
            java[0].failures.put("utxos", new IllegalStateException("backend bug"));
            assertEquals("INTERNAL_ERROR", c.fails("utxo.list", obj("account_id", WPKH)));
            java[0].failures.clear();
            c.ok("utxo.list", obj("account_id", WPKH));
        }
    }

    @Test
    void failingInventoryReadsAsEmpty() throws Exception {
        ForwardingBackend[] java = new ForwardingBackend[1];
        try (Harness h = Harness.start(Config.defaults(), m -> java[0] = new ForwardingBackend(m))) {
            LocalClient c = h.client(READ_SCOPES).client();
            java[0].failures.put("wallets", new IllegalStateException("backend bug"));
            assertEquals(0, c.ok("wallet.list", obj()).get("wallets").size());
        }
    }
}
