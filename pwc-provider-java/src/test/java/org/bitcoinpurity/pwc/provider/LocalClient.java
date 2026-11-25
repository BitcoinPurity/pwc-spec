package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.UncheckedIOException;
import java.net.UnixDomainSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.AsynchronousFileChannel;
import java.nio.channels.Channels;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileSystemException;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.concurrent.ExecutionException;
import java.security.GeneralSecurityException;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;

/** A LOCAL Client for tests: JSON-RPC over the Unix socket or the Windows pipe, with events kept aside. */
final class LocalClient implements AutoCloseable {
    /** The connection: a Unix socket, or a Windows pipe opened for overlapped I/O so that reads and writes do not
     * wait for each other. */
    private interface Transport extends AutoCloseable {
        void write(ByteBuffer b) throws IOException;

        InputStream input();

        @Override
        void close() throws IOException;
    }

    private static Transport unix(Path socket) throws IOException {
        SocketChannel ch = SocketChannel.open(UnixDomainSocketAddress.of(socket));
        return new Transport() {
            public void write(ByteBuffer b) throws IOException {
                while (b.hasRemaining()) {
                    ch.write(b);
                }
            }

            public InputStream input() {
                return Channels.newInputStream(ch);
            }

            public void close() throws IOException {
                ch.close();
            }
        };
    }

    private static Transport pipe(Path name) throws IOException {
        AsynchronousFileChannel ch = openPipe(name);
        return new Transport() {
            public void write(ByteBuffer b) throws IOException {
                while (b.hasRemaining()) {
                    get(ch.write(b, 0));
                }
            }

            public InputStream input() {
                return new InputStream() {
                    public int read() throws IOException {
                        byte[] one = new byte[1];
                        return read(one, 0, 1) < 0 ? -1 : one[0] & 0xff;
                    }

                    public int read(byte[] b, int off, int len) throws IOException {
                        try {
                            return get(ch.read(ByteBuffer.wrap(b, off, len), 0));
                        } catch (IOException e) {
                            return -1; // the pipe was closed
                        }
                    }
                };
            }

            public void close() throws IOException {
                ch.close();
            }
        };
    }

    private static AsynchronousFileChannel openPipe(Path name) throws IOException {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
        while (true) {
            try {
                return AsynchronousFileChannel.open(name, StandardOpenOption.READ, StandardOpenOption.WRITE);
            } catch (FileSystemException busy) {
                if (System.nanoTime() > deadline) {
                    throw busy;
                }
                try {
                    Thread.sleep(10);
                } catch (InterruptedException e) {
                    throw new IOException(e);
                }
            }
        }
    }

    private static int get(java.util.concurrent.Future<Integer> f) throws IOException {
        try {
            return f.get();
        } catch (ExecutionException e) {
            throw e.getCause() instanceof IOException io ? io : new IOException(e.getCause());
        } catch (InterruptedException e) {
            throw new IOException(e);
        }
    }

    static final ObjectMapper JSON = new ObjectMapper();
    static final HexFormat HEX = HexFormat.of();
    private static final JsonNode EOF = JSON.createObjectNode().put("eof", true);

    private final Transport channel;
    private final BlockingQueue<JsonNode> frames = new LinkedBlockingQueue<>();
    private final List<JsonNode> events = new ArrayList<>();
    private long nextId = 1;

    LocalClient(Path socket) throws IOException {
        channel = Harness.WINDOWS ? pipe(socket) : unix(socket);
        Thread reader = new Thread(this::read, "test-client-reader");
        reader.setDaemon(true);
        reader.start();
    }

    private void read() {
        try (BufferedReader in = new BufferedReader(new InputStreamReader(channel.input(), StandardCharsets.UTF_8))) {
            for (String line; (line = in.readLine()) != null; ) {
                frames.add(JSON.readTree(line));
            }
        } catch (IOException e) {
            // The connection closed.
        }
        frames.add(EOF);
    }

    static ObjectNode obj(Object... keysAndValues) {
        ObjectNode o = JSON.createObjectNode();
        for (int i = 0; i < keysAndValues.length; i += 2) {
            o.set((String) keysAndValues[i], JSON.valueToTree(keysAndValues[i + 1]));
        }
        return o;
    }

    void send(JsonNode frame) {
        try {
            channel.write(ByteBuffer.wrap((JSON.writeValueAsString(frame) + "\n").getBytes(StandardCharsets.UTF_8)));
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    long sendRequest(String method, JsonNode params) {
        long id = nextId++;
        send(obj("jsonrpc", "2.0", "id", id, "method", method, "params", params));
        return id;
    }

    /** The next frame, or null at the end of the stream. */
    JsonNode recv() {
        try {
            JsonNode f = frames.poll(5, TimeUnit.SECONDS);
            assertNotNull(f, "a frame in time");
            return f == EOF ? null : f;
        } catch (InterruptedException e) {
            throw new IllegalStateException(e);
        }
    }

    /** True if the Provider closes the connection without sending another frame. */
    boolean closed() {
        return recv() == null;
    }

    JsonNode response(long id) {
        while (true) {
            JsonNode f = recv();
            assertNotNull(f, "connection open");
            if ("pwc.event".equals(f.path("method").asText())) {
                events.add(f.get("params"));
                continue;
            }
            assertEquals(id, f.get("id").asLong(), "unexpected frame " + f);
            return f;
        }
    }

    JsonNode call(String method, JsonNode params) {
        return response(sendRequest(method, params));
    }

    JsonNode ok(String method, JsonNode params) {
        JsonNode r = call(method, params);
        if (!r.has("result")) {
            throw new AssertionError(method + " failed: " + r);
        }
        return r.get("result");
    }

    String fails(String method, JsonNode params) {
        JsonNode r = call(method, params);
        if (!r.has("error")) {
            throw new AssertionError(method + " unexpectedly succeeded: " + r);
        }
        return code(r);
    }

    static String code(JsonNode response) {
        return response.path("error").path("data").path("code").asText();
    }

    JsonNode nextEvent() {
        if (!events.isEmpty()) {
            return events.remove(0);
        }
        while (true) {
            JsonNode f = recv();
            assertNotNull(f, "connection open");
            if ("pwc.event".equals(f.path("method").asText())) {
                return f.get("params");
            }
        }
    }

    /** Fails if a frame arrives within a short time. */
    void noEvent() throws InterruptedException {
        if (!events.isEmpty()) {
            throw new AssertionError("unexpected events " + events);
        }
        JsonNode f = frames.poll(200, TimeUnit.MILLISECONDS);
        if (f != null) {
            throw new AssertionError("unexpected frame " + f);
        }
    }

    JsonNode hello() {
        return ok("local.hello", obj("protocols", List.of("pwc-local/1")));
    }

    /** The raw response to {@code local.authenticate}; a successful one has its server proof checked. */
    JsonNode authenticate(JsonNode hello, String clientId, byte[] secret) {
        byte[] serverNonce = HEX.parseHex(hello.get("server_nonce").asText());
        byte[] clientNonce = new byte[32];
        java.util.Arrays.fill(clientNonce, (byte) 7);
        byte[] proof = proof(secret, "PWC-LOCAL-1 client", serverNonce, clientNonce, clientId);
        JsonNode r = call(
                "local.authenticate",
                obj("client_id", clientId, "client_nonce", HEX.formatHex(clientNonce), "client_proof", HEX.formatHex(proof)));
        if (r.has("result")) {
            byte[] expected = proof(secret, "PWC-LOCAL-1 provider", clientNonce, serverNonce, clientId);
            assertEquals(HEX.formatHex(expected), r.get("result").get("server_proof").asText());
        }
        return r;
    }

    private static byte[] proof(byte[] key, String label, byte[] first, byte[] second, String clientId) {
        try {
            Mac mac = Mac.getInstance("HmacSHA256");
            mac.init(new SecretKeySpec(key, "HmacSHA256"));
            mac.update(label.getBytes(StandardCharsets.US_ASCII));
            mac.update((byte) 0);
            mac.update(first);
            mac.update(second);
            return mac.doFinal(clientId.getBytes(StandardCharsets.UTF_8));
        } catch (GeneralSecurityException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Polls {@code operation.get} until the Operation is terminal. */
    JsonNode waitFor(String operationId) throws InterruptedException {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
        while (true) {
            JsonNode op = ok("operation.get", obj("operation_id", operationId));
            String s = op.get("state").asText();
            if (List.of("completed", "rejected", "expired", "canceled", "failed").contains(s)) {
                return op;
            }
            if (System.nanoTime() > deadline) {
                throw new AssertionError("Operation " + operationId + " still " + s);
            }
            Thread.sleep(20);
        }
    }

    @Override
    public void close() throws IOException {
        channel.close();
    }
}
