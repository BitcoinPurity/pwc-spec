package org.bitcoinpurity.pwc.provider;

import static org.bitcoinpurity.pwc.provider.LocalClient.code;
import static org.bitcoinpurity.pwc.provider.LocalClient.obj;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.Duration;
import java.time.Instant;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

class PairingTest {
    @Test
    void approvedPairingPromptCarriesTheClaimsAndThePeer() throws Exception {
        try (Harness h = Harness.start()) {
            LocalClient c = h.connect();
            c.hello();
            long id = c.sendRequest(
                    "local.pair",
                    obj("client_name", "Example Trader", "client_description", "Trades", "pairing_code", "482913"));
            PairingPrompt p = h.nextPrompt(PairingPrompt.class);
            PairingRequest r = p.request();
            assertEquals("Example Trader", r.claimedName());
            assertEquals("Trades", r.claimedDescription());
            assertEquals("482913", r.pairingCode());
            assertEquals((int) ProcessHandle.current().pid(), r.peer().pid());
            assertNotNull(r.peer().executable());
            assertTrue(p.toString().contains("Example Trader") && p.toString().contains("unverified"));
            assertTrue(p.toString().contains("482913") && p.toString().contains(String.valueOf(r.peer().pid())));
            assertFalse(p.isWithdrawn());
            p.approve();
            assertThrows(IllegalStateException.class, p::approve);

            var result = c.response(id).get("result");
            String clientId = result.get("client_id").asText();
            assertTrue(clientId.startsWith("cl-"));
            assertEquals(clientId, c.ok("session.getInfo", obj()).get("client_id").asText());

            var clients = h.provider.pairedClients();
            assertEquals(1, clients.size());
            PairedClient pc = clients.get(0);
            assertEquals(clientId, pc.clientId());
            assertEquals("Example Trader", pc.claimedName());
            assertEquals("Trades", pc.claimedDescription());
            assertEquals(r.peer(), pc.peer());
            assertTrue(Duration.between(pc.pairedAt(), Instant.now()).abs().toSeconds() < 60);
            assertNotNull(pc.lastConnectedAt());

            byte[] secret = LocalClient.HEX.parseHex(result.get("client_secret").asText());
            h.login(clientId, secret).close();
        }
    }

    @Test
    void rejectedPairing() throws Exception {
        try (Harness h = Harness.start()) {
            LocalClient c = h.connect();
            c.hello();
            long id = c.sendRequest("local.pair", obj("client_name", "App", "pairing_code", "111111"));
            PairingPrompt p = h.nextPrompt(PairingPrompt.class);
            p.reject();
            assertThrows(IllegalStateException.class, p::reject);
            assertEquals("USER_REJECTED", code(c.response(id)));
            assertTrue(h.provider.pairedClients().isEmpty());
        }
    }

    @Test
    void promptWithdrawnWhenTheConnectionCloses() throws Exception {
        try (Harness h = Harness.start()) {
            LocalClient c = h.connect();
            c.hello();
            c.sendRequest("local.pair", obj("client_name", "App", "pairing_code", "111111"));
            PairingPrompt p = h.nextPrompt(PairingPrompt.class);
            c.close();
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
            while (!p.isWithdrawn()) {
                assertTrue(System.nanoTime() < deadline, "prompt withdrawn");
                Thread.sleep(10);
            }
            p.approve();
            assertTrue(h.provider.pairedClients().isEmpty());
        }
    }

    @Test
    void unansweredPromptExpires() throws Exception {
        try (Harness h = Harness.start(Config.defaults().toBuilder().pairingTimeout(Duration.ofMillis(200)).build())) {
            LocalClient c = h.connect();
            c.hello();
            long id = c.sendRequest("local.pair", obj("client_name", "App", "pairing_code", "111111"));
            PairingPrompt p = h.nextPrompt(PairingPrompt.class);
            assertEquals("OPERATION_EXPIRED", code(c.response(id)));
            assertTrue(p.isWithdrawn());
        }
    }

    @Test
    void throwingHandlerRefusesThePrompt() throws Exception {
        try (MemoryBackend memory = MemoryBackend.demo();
                Provider provider = Provider.create(memory, Config.defaults(), p -> {
                    throw new IllegalStateException("handler failed");
                });
                LocalServer server = Harness.start(provider, java.nio.file.Files.createTempDirectory("pwc"))) {
            LocalClient c = new LocalClient(server.path());
            c.hello();
            assertEquals("USER_REJECTED", c.fails("local.pair", obj("client_name", "App", "pairing_code", "111111")));
            c.close();
        }
    }

    @Test
    void unpairClosesTheConnectionsAndForgetsTheClient() throws Exception {
        try (Harness h = Harness.start()) {
            Harness.Paired p = h.pair();
            LocalClient second = h.login(p.clientId(), p.secret());
            h.provider.unpairClient(p.clientId());
            assertTrue(p.client().closed());
            assertTrue(second.closed());
            assertTrue(h.provider.pairedClients().isEmpty());
            LocalClient again = h.connect();
            assertEquals("UNAUTHENTICATED", code(again.authenticate(again.hello(), p.clientId(), p.secret())));
        }
    }
}
