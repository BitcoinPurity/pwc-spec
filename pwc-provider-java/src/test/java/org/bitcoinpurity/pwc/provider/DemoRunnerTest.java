package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Runs the LOCAL conformance runner of {@code pwc-testkit/} against the Java demo program. */
class DemoRunnerTest {
    @Test
    void localRunnerReportsNoFailures(@TempDir Path tmp) throws Exception {
        Path run = tmp.resolve("run");
        String java = ProcessHandle.current().info().command().orElseThrow();
        Process provider = new ProcessBuilder(
                        java, "--enable-native-access=ALL-UNNAMED", "-cp", System.getProperty("java.class.path"),
                        "org.bitcoinpurity.pwc.provider.demo.Main",
                        "--dir", run.toString(), "--approve", "auto", "--approve-delay-ms", "300")
                .redirectOutput(ProcessBuilder.Redirect.DISCARD)
                .redirectError(ProcessBuilder.Redirect.DISCARD)
                .start();
        try {
            Path socket = run.resolve("pwc-provider.sock");
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
            while (!Files.exists(socket)) {
                assertTrue(System.nanoTime() < deadline && provider.isAlive(), "Provider did not start");
                Thread.sleep(20);
            }
            Path report = tmp.resolve("report.json");
            Path runner = Path.of(System.getProperty("pwc.testkit"), "local_runner.py");
            Process r = new ProcessBuilder(
                            "python3", runner.toString(), "--socket", socket.toString(), "--pair",
                            "--allow-destructive", "--report", report.toString())
                    .redirectErrorStream(true)
                    .start();
            String output = new String(r.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            assertEquals(0, r.waitFor(), output);

            List<String> outcomes = new ArrayList<>();
            for (JsonNode result : LocalClient.JSON.readTree(report.toFile()).get("results")) {
                outcomes.add(result.get("id").asText() + "=" + result.get("outcome").asText());
            }
            assertTrue(outcomes.stream().allMatch(o -> o.endsWith("=pass") || o.endsWith("=manual")), outcomes.toString());
            assertEquals(16, outcomes.stream().filter(o -> o.endsWith("=pass")).count(), output);
        } finally {
            provider.destroy();
            provider.waitFor();
        }
    }
}
