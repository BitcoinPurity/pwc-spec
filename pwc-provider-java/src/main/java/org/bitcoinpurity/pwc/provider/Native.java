package org.bitcoinpurity.pwc.provider;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import org.bitcoinj.base.Sha256Hash;
import org.bitcoinj.core.Transaction;
import org.bitcoinj.core.TransactionOutPoint;
import org.bitcoinj.core.TransactionOutput;
import org.bitcoinj.script.Script;

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

    // Provider

    static native long providerCreate(long memory, WalletBackend backend, Config config, Provider.Dispatcher prompts);

    static native long providerRestore(
            long memory, WalletBackend backend, Config config, Provider.Dispatcher prompts, byte[] snapshot)
            throws StateException;

    static native void providerFree(long provider);

    static native byte[] providerSaveState(long provider);

    static native List<PairedClient> providerPairedClients(long provider);

    static native List<Grant> providerGrants(long provider);

    static native boolean providerRevokeGrant(long provider, String grantId);

    static native void providerUnpairClient(long provider, String clientId);

    static native void providerNotify(long provider, BackendEvent event);

    // LOCAL server

    static native String serverDefaultDirectory();

    static native long serverStart(long provider, String dir, String providerId) throws EndpointException;

    static native String serverPath(long server);

    static native void serverClose(long server);

    // Prompts

    static native boolean promptIsWithdrawn(long prompt);

    static native boolean promptAnswer(long prompt, boolean approve);

    static native boolean promptGrant(long prompt, GrantChoice choice);

    static native void promptFree(long prompt);

    // In-memory backend

    static native long memoryDemo();

    static native void memoryFree(long memory);

    static native String memoryAddWallet(long memory, String walletId, String name, String network, WalletKind kind);

    static native String memoryAddAccount(
            long memory, String walletId, String accountId, String name, ScriptType scriptType, boolean canSign);

    static native void memorySetWalletState(long memory, String walletId, WalletState state);

    static native Sha256Hash memoryReceive(long memory, String accountId, long value, int confirmations, boolean coinbase);

    static native Sha256Hash memoryReceiveConflicting(long memory, String accountId, Sha256Hash txid, long value);

    static native void memoryReplace(long memory, String network, Sha256Hash txid, Sha256Hash by);

    static native void memoryDropPending(long memory, String network, Sha256Hash txid);

    static native void memoryMine(long memory, String network, int blocks);

    static native void memoryReorg(long memory, String network, int depth);

    static native void memorySetOffline(long memory, String network, boolean offline);

    static native void memorySetFeeEstimate(long memory, String network, boolean present, long satPerKvb);

    static native Sha256Hash memorySpendByUser(long memory, String accountId, TransactionOutPoint outpoint, long value);

    static native int memoryTip(long memory, String network);

    static native List<WalletInfo> memoryWallets(long memory);

    static native AccountInfo memoryAccount(long memory, String accountId);

    static native Balance memoryBalance(long memory, String accountId, Set<TransactionOutPoint> reserved)
            throws PwcException;

    static native AddressRecord memoryReceiveAddress(long memory, String accountId) throws PwcException;

    static native AddressRecord memoryCreateAddress(long memory, String accountId, String label) throws PwcException;

    static native List<AddressRecord> memoryAddresses(long memory, String accountId);

    static native List<Utxo> memoryUtxos(long memory, String accountId, Set<TransactionOutPoint> reserved)
            throws PwcException;

    static native List<TransactionRecord> memoryTransactions(long memory, String accountId) throws PwcException;

    static native boolean memoryOwnsScript(long memory, String accountId, Script script);

    static native boolean memoryIsUnspent(long memory, String accountId, TransactionOutPoint outpoint);

    static native Long memoryFeeEstimate(long memory, String network, long targetBlocks);

    static native CreatedPsbt memoryCreatePsbt(
            long memory, String accountId, CreateRequest request, Set<TransactionOutPoint> excluded)
            throws PwcException;

    static native TransactionOutput memoryOwnedOutput(long memory, String accountId, TransactionOutPoint outpoint);

    static native SignedPsbt memorySignPsbt(long memory, String accountId, byte[] psbt, List<Integer> inputs)
            throws PwcException;

    static native byte[] memorySignMessage(
            long memory, String accountId, Script script, String message, MessageFormat format) throws PwcException;

    static native Sha256Hash memoryBroadcast(long memory, String network, Transaction tx) throws PwcException;
}
