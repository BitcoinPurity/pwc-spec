package org.bitcoinpurity.pwc.provider;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;

/** The native methods of the JNI library, which is loaded from the jar. */
final class Native {
    static {
        load();
    }

    private Native() {}

    @SuppressWarnings("restricted")
    private static void load() {
        String osName = System.getProperty("os.name").toLowerCase(Locale.ROOT);
        String os = osName.startsWith("mac") ? "macos" : osName;
        String arch = switch (System.getProperty("os.arch")) {
            case "amd64" -> "x86_64";
            case "arm64" -> "aarch64";
            default -> System.getProperty("os.arch");
        };
        String file = os.equals("macos") ? "libpwc_provider_jni.dylib" : "libpwc_provider_jni.so";
        String resource = "native/" + os + "-" + arch + "/" + file;
        try (InputStream in = Native.class.getResourceAsStream(resource)) {
            if (in == null) {
                throw new UnsatisfiedLinkError("this jar has no native library for " + os + "-" + arch);
            }
            Path copy = Files.createTempFile("pwc_provider_jni", file.substring(file.lastIndexOf('.')));
            copy.toFile().deleteOnExit();
            Files.copy(in, copy, StandardCopyOption.REPLACE_EXISTING);
            System.load(copy.toString());
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    static native Config defaultConfig();
}
