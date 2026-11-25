package org.bitcoinpurity.pwc.provider.demo;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.Charset;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import org.bitcoinpurity.pwc.provider.Config;
import org.bitcoinpurity.pwc.provider.EndpointException;
import org.bitcoinpurity.pwc.provider.GrantChoice;
import org.bitcoinpurity.pwc.provider.LocalServer;
import org.bitcoinpurity.pwc.provider.MemoryBackend;
import org.bitcoinpurity.pwc.provider.MessageSignPrompt;
import org.bitcoinpurity.pwc.provider.PairingPrompt;
import org.bitcoinpurity.pwc.provider.PermissionPrompt;
import org.bitcoinpurity.pwc.provider.PermissionRequest;
import org.bitcoinpurity.pwc.provider.Prompt;
import org.bitcoinpurity.pwc.provider.PromptHandler;
import org.bitcoinpurity.pwc.provider.Provider;
import org.bitcoinpurity.pwc.provider.PsbtSignPrompt;
import org.bitcoinpurity.pwc.provider.WalletBackend;
import org.bitcoinpurity.pwc.provider.WalletInfo;

/**
 * Demo PWC LOCAL Provider backed by the in-memory wallet.
 *
 * <p>Usage: {@code Main [--dir DIR] [--approve ask|auto] [--approve-delay-ms N]}; {@code --dir} is for macOS and
 * Linux, Windows endpoints being named pipes.
 */
public final class Main {
    private Main() {}

    private static void usage() {
        System.err.println("usage: pwc-provider [--dir DIR] [--approve ask|auto] [--approve-delay-ms N]");
        System.exit(2);
    }

    /**
     * Grants what was requested: the requested network or {@code regtest}, and the requested wallets and accounts
     * or every wallet of that network.
     */
    static GrantChoice asRequested(WalletBackend backend, PermissionRequest r) {
        String network = r.network() == null ? "regtest" : r.network();
        List<String> wallets = r.walletIds();
        List<String> accounts = r.accountIds();
        if (wallets.isEmpty() && accounts.isEmpty()) {
            wallets = backend.wallets().stream()
                    .filter(w -> w.network().equals(network))
                    .map(WalletInfo::walletId)
                    .toList();
        }
        return new GrantChoice(network, r.scopes(), wallets, accounts, r.expiresAt(), r.constraints());
    }

    static void answer(WalletBackend backend, Prompt prompt, boolean yes) {
        if (!yes) {
            prompt.reject();
            return;
        }
        switch (prompt) {
            case PairingPrompt p -> p.approve();
            case PermissionPrompt p -> p.grant(asRequested(backend, p.request()));
            case PsbtSignPrompt p -> p.approve();
            case MessageSignPrompt p -> p.approve();
        }
    }

    static PromptHandler approveAutomatically(WalletBackend backend, long delayMillis) {
        ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread t = new Thread(r, "auto-approve");
            t.setDaemon(true);
            return t;
        });
        return prompt -> {
            System.err.println("[auto-approve]\n" + prompt);
            timer.schedule(() -> answer(backend, prompt, true), delayMillis, TimeUnit.MILLISECONDS);
        };
    }

    static PromptHandler ask(WalletBackend backend) {
        BufferedReader stdin = new BufferedReader(new InputStreamReader(System.in, Charset.defaultCharset()));
        return prompt -> {
            System.err.println("\n" + prompt + "\nApprove? [y/N] ");
            String line;
            try {
                line = stdin.readLine();
            } catch (IOException e) {
                line = null;
            }
            answer(backend, prompt, line != null && line.trim().equalsIgnoreCase("y"));
        };
    }

    public static void main(String[] args) throws InterruptedException {
        Path dir = null;
        boolean auto = false;
        long delay = 300;
        for (int i = 0; i < args.length; i += 2) {
            if (i + 1 >= args.length) {
                usage();
            }
            String value = args[i + 1];
            switch (args[i]) {
                case "--dir" -> dir = Path.of(value);
                case "--approve" -> {
                    switch (value) {
                        case "ask" -> auto = false;
                        case "auto" -> auto = true;
                        default -> usage();
                    }
                }
                case "--approve-delay-ms" -> {
                    try {
                        delay = Long.parseLong(value);
                    } catch (NumberFormatException e) {
                        usage();
                    }
                }
                default -> usage();
            }
        }
        MemoryBackend backend = MemoryBackend.demo();
        PromptHandler prompts = auto ? approveAutomatically(backend, delay) : ask(backend);
        Provider provider = Provider.create(backend, Config.defaults(), prompts);
        LocalServer server;
        try {
            server = dir == null
                    ? LocalServer.start(provider)
                    : LocalServer.start(provider, dir, LocalServer.PROVIDER_ID);
        } catch (EndpointException | UnsupportedOperationException e) {
            System.err.println("pwc-provider: " + e.getMessage());
            System.exit(1);
            return;
        }
        System.err.println("pwc-provider listening on " + server.path());
        if (auto) {
            System.err.println(
                    "WARNING: --approve auto approves every pairing and request without asking. Test use only.");
        }
        Runtime.getRuntime().addShutdownHook(new Thread(server::close));
        new CountDownLatch(1).await();
    }
}
