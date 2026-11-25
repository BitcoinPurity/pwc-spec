package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.InputStream;
import java.lang.module.ModuleDescriptor;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class NativeLoadingTest {
    /** Prints where a fresh JVM loaded the native library from. */
    public static void main(String[] args) {
        Config.defaults();
        System.out.println(Native.loadedFrom);
    }

    private static String loadedFromInFreshJvm(String... jvmArgs) throws Exception {
        List<String> command = new ArrayList<>();
        command.add(ProcessHandle.current().info().command().orElseThrow());
        command.add("--enable-native-access=ALL-UNNAMED");
        command.addAll(List.of(jvmArgs));
        command.addAll(List.of("-cp", System.getProperty("java.class.path"), NativeLoadingTest.class.getName()));
        Process p = new ProcessBuilder(command).redirectErrorStream(true).start();
        String out = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
        assertEquals(0, p.waitFor(), out);
        return out.lines().reduce((a, b) -> b).orElse("");
    }

    @Test
    void loadsFromTheJarByDefault() throws Exception {
        assertEquals("jar", loadedFromInFreshJvm());
    }

    @Test
    void prefersJavaLibraryPath(@TempDir Path dir) throws Exception {
        String file = System.mapLibraryName("pwc_provider_jni");
        try (InputStream in = Native.class.getResourceAsStream("native/" + Native.platform() + "/" + file)) {
            Files.copy(in, dir.resolve(file));
        }
        assertEquals("java.library.path", loadedFromInFreshJvm("-Djava.library.path=" + dir));
    }

    @Test
    void theSdkIsAModuleWithoutDependencies() throws Exception {
        Path descriptor = Path.of("build", "classes", "java", "main", "module-info.class");
        try (InputStream in = Files.newInputStream(descriptor)) {
            ModuleDescriptor m = ModuleDescriptor.read(in);
            assertEquals("org.bitcoinpurity.pwc.provider", m.name());
            assertEquals(
                    Set.of("org.bitcoinpurity.pwc.provider"),
                    m.exports().stream().map(ModuleDescriptor.Exports::source).collect(Collectors.toSet()));
            assertEquals(
                    Set.of("java.base"),
                    m.requires().stream().map(ModuleDescriptor.Requires::name).collect(Collectors.toSet()));
        }
        try (Stream<Path> files = Files.list(Path.of("build", "classes", "java", "main"))) {
            assertTrue(files.anyMatch(f -> f.getFileName().toString().equals("org")));
        }
    }
}
