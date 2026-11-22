package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.net.UnixDomainSocketAddress;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class LocalServerTest {
    @TempDir
    Path tmp;

    @Test
    void endpointDirectoryAndSocketPermissions() throws Exception {
        try (MemoryBackend memory = MemoryBackend.demo();
                Provider provider = Provider.create(memory, Config.defaults(), Prompt::reject)) {
            LocalServer server = LocalServer.start(provider, tmp.resolve("run"));
            assertEquals(tmp.resolve("run").resolve("pwc-provider.sock"), server.path());
            assertEquals("rwx------", PosixFilePermissions.toString(Files.getPosixFilePermissions(tmp.resolve("run"))));
            assertEquals("rw-------", PosixFilePermissions.toString(Files.getPosixFilePermissions(server.path())));
            server.close();
            assertFalse(Files.exists(tmp.resolve("run").resolve("pwc-provider.sock")), "socket removed by close");
        }
    }

    @Test
    void endpointErrors() throws Exception {
        Path open = Files.createDirectory(tmp.resolve("open"));
        Files.setPosixFilePermissions(open, PosixFilePermissions.fromString("rwxr-xr-x"));
        try (MemoryBackend memory = MemoryBackend.demo();
                Provider provider = Provider.create(memory, Config.defaults(), Prompt::reject)) {
            EndpointException unsafe = assertThrows(EndpointException.class, () -> LocalServer.start(provider, open));
            assertEquals(EndpointException.Kind.UNSAFE_DIRECTORY, unsafe.kind());
            assertEquals(open, unsafe.path());

            EndpointException tooLong = assertThrows(
                    EndpointException.class, () -> LocalServer.start(provider, tmp.resolve("d".repeat(120))));
            assertEquals(EndpointException.Kind.PATH_TOO_LONG, tooLong.kind());

            try (LocalServer first = LocalServer.start(provider, tmp.resolve("run"), "other-id")) {
                EndpointException running = assertThrows(
                        EndpointException.class, () -> LocalServer.start(provider, tmp.resolve("run"), "other-id"));
                assertEquals(EndpointException.Kind.ALREADY_RUNNING, running.kind());
                assertEquals(first.path(), running.path());
                assertTrue(running.getMessage().contains("another Provider instance"));
                SocketChannel.open(UnixDomainSocketAddress.of(first.path())).close();
            }
        }
    }

    @Test
    void staleSocketReplaced() throws Exception {
        Path dir = Files.createDirectory(tmp.resolve("run"), PosixFilePermissions.asFileAttribute(
                PosixFilePermissions.fromString("rwx------")));
        Path socket = dir.resolve("pwc-provider.sock");
        ServerSocketChannel.open(java.net.StandardProtocolFamily.UNIX)
                .bind(UnixDomainSocketAddress.of(socket))
                .close();
        assertTrue(Files.exists(socket));
        try (MemoryBackend memory = MemoryBackend.demo();
                Provider provider = Provider.create(memory, Config.defaults(), Prompt::reject);
                LocalServer server = LocalServer.start(provider, dir)) {
            SocketChannel.open(UnixDomainSocketAddress.of(server.path())).close();
        }
    }

    @Test
    void defaultDirectoryIsUnderTheUsersDirectories() {
        Path dir = LocalServer.defaultDirectory().orElseThrow();
        String runtime = System.getenv("XDG_RUNTIME_DIR");
        assertTrue(dir.endsWith("pwc") && runtime != null && dir.startsWith(runtime)
                || dir.endsWith(Path.of(".pwc", "run")), dir.toString());
        assertEquals("pwc-provider", LocalServer.PROVIDER_ID);
    }
}
