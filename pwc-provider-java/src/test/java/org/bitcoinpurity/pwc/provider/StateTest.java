package org.bitcoinpurity.pwc.provider;

import static org.bitcoinpurity.pwc.provider.LocalClient.obj;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.nio.charset.StandardCharsets;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import org.junit.jupiter.api.Test;

class StateTest {
    @Test
    void restoredProviderKeepsCredentialsAndGrants() throws Exception {
        try (Harness h = Harness.start()) {
            Harness.Paired p = h.client("wallets:read");
            byte[] snapshot = h.provider.saveState();
            BlockingQueue<Prompt> prompts = new LinkedBlockingQueue<>();
            Config config = Config.defaults().toBuilder().providerName("Restored").build();
            try (Provider restored = Provider.withState(h.memory, config, prompts::add, snapshot);
                    Harness again = Harness.serve(h.memory, restored, prompts)) {
                assertEquals(config, restored.config());
                assertEquals(h.provider.pairedClients(), restored.pairedClients());
                assertEquals(h.provider.grants(), restored.grants());
                LocalClient c = again.login(p.clientId(), p.secret());
                assertEquals(2, c.ok("wallet.list", obj()).get("wallets").size());
                assertEquals("Restored", c.ok("pwc.getCapabilities", obj()).get("provider").get("name").asText());
            }
        }
    }

    @Test
    void corruptOrUnknownSnapshotsAreRefused() throws Exception {
        try (MemoryBackend memory = MemoryBackend.demo()) {
            StateException corrupt = assertThrows(StateException.class, () -> Provider.withState(
                    memory, Config.defaults(), Prompt::reject, "not json".getBytes(StandardCharsets.UTF_8)));
            assertEquals(StateException.Kind.CORRUPT, corrupt.kind());
            StateException version = assertThrows(StateException.class, () -> Provider.withState(
                    memory, Config.defaults(), Prompt::reject, "{\"version\":99}".getBytes(StandardCharsets.UTF_8)));
            assertEquals(StateException.Kind.UNSUPPORTED_VERSION, version.kind());
            assertEquals("unsupported Provider state version 99", version.getMessage());
        }
    }
}
