package org.bitcoinpurity.pwc.provider;

import static org.bitcoinpurity.pwc.provider.LocalClient.obj;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.function.Function;
import java.util.stream.Stream;

/** A Provider on the in-memory wallet, served on a temporary endpoint; prompts are queued for the test. */
final class Harness implements AutoCloseable {
    record Paired(LocalClient client, String clientId, byte[] secret) {}

    final Path tmp;
    final MemoryBackend memory;
    final Provider provider;
    final LocalServer server;
    final BlockingQueue<Prompt> prompts;
    private final boolean ownsMemory;

    private Harness(MemoryBackend memory, Provider provider, BlockingQueue<Prompt> prompts, boolean ownsMemory)
            throws Exception {
        this.tmp = Files.createTempDirectory("pwc");
        this.memory = memory;
        this.ownsMemory = ownsMemory;
        this.provider = provider;
        this.prompts = prompts;
        this.server = LocalServer.start(provider, tmp.resolve("run"));
    }

    static Harness start() throws Exception {
        return start(Config.defaults(), m -> m);
    }

    static Harness start(Config config) throws Exception {
        return start(config, m -> m);
    }

    /** A Provider on {@code backend(memory)}, for example a Java backend that wraps the in-memory one. */
    static Harness start(Config config, Function<MemoryBackend, WalletBackend> backend) throws Exception {
        MemoryBackend memory = MemoryBackend.demo();
        BlockingQueue<Prompt> prompts = new LinkedBlockingQueue<>();
        return new Harness(memory, Provider.create(backend.apply(memory), config, prompts::add), prompts, true);
    }

    /** Serves an existing Provider whose prompts arrive in {@code prompts}; {@code memory} stays open. */
    static Harness serve(MemoryBackend memory, Provider provider, BlockingQueue<Prompt> prompts) throws Exception {
        return new Harness(memory, provider, prompts, false);
    }

    Path socket() {
        return server.path();
    }

    Prompt nextPrompt() throws InterruptedException {
        Prompt p = prompts.poll(5, TimeUnit.SECONDS);
        assertNotNull(p, "a prompt");
        return p;
    }

    <T extends Prompt> T nextPrompt(Class<T> kind) throws InterruptedException {
        return kind.cast(nextPrompt());
    }

    void noPrompt() throws InterruptedException {
        assertNull(prompts.poll(200, TimeUnit.MILLISECONDS), "unexpected prompt");
    }

    LocalClient connect() throws IOException {
        return new LocalClient(socket());
    }

    /** Pairs a new Client, approving the pairing prompt. */
    Paired pair() throws Exception {
        LocalClient c = connect();
        c.hello();
        long id = c.sendRequest("local.pair", obj("client_name", "Test client", "pairing_code", "123456"));
        nextPrompt(PairingPrompt.class).approve();
        JsonNode r = c.response(id).get("result");
        return new Paired(c, r.get("client_id").asText(), LocalClient.HEX.parseHex(r.get("client_secret").asText()));
    }

    /** Opens a connection and authenticates a paired Client. */
    LocalClient login(String clientId, byte[] secret) throws IOException {
        LocalClient c = connect();
        JsonNode r = c.authenticate(c.hello(), clientId, secret);
        assertNotNull(r.get("result"), "authentication failed: " + r);
        return c;
    }

    /** What a User grants when approving exactly what was asked (as the Rust tests and the demo do). */
    GrantChoice asRequested(PermissionRequest r) {
        String network = r.network() == null ? "regtest" : r.network();
        List<String> wallets = r.walletIds();
        List<String> accounts = r.accountIds();
        if (wallets.isEmpty() && accounts.isEmpty()) {
            wallets = memory.wallets().stream()
                    .filter(w -> w.network().equals(network))
                    .map(WalletInfo::walletId)
                    .toList();
        }
        return new GrantChoice(network, r.scopes(), wallets, accounts, r.expiresAt(), r.constraints());
    }

    /** Requests {@code params} and approves them as requested; returns the completed Operation. */
    JsonNode grant(LocalClient c, ObjectNode params) throws Exception {
        if (!params.has("idempotency_key")) {
            params.put("idempotency_key", "k-" + UUID.randomUUID());
        }
        JsonNode op = c.ok("session.requestPermissions", params);
        PermissionPrompt p = nextPrompt(PermissionPrompt.class);
        p.grant(asRequested(p.request()));
        JsonNode done = c.waitFor(op.get("operation_id").asText());
        assertEquals("completed", done.get("state").asText(), done.toString());
        return done;
    }

    /** Pairs a Client and grants it {@code scopes} on every regtest wallet. */
    Paired client(String... scopes) throws Exception {
        Paired p = pair();
        if (scopes.length > 0) {
            grant(p.client(), obj("scopes", List.of(scopes)));
            if (List.of(scopes).contains("events:read")) {
                assertEquals("operation.stateChanged", p.client().nextEvent().get("event_type").asText());
            }
        }
        return p;
    }

    @Override
    public void close() throws IOException {
        server.close();
        provider.close();
        if (ownsMemory) {
            memory.close();
        }
        try (Stream<Path> files = Files.walk(tmp)) {
            files.sorted(Comparator.reverseOrder()).forEach(p -> p.toFile().delete());
        }
    }
}
