package org.bitcoinpurity.pwc.provider;

import java.lang.ref.Reference;
import java.util.List;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.Set;

/**
 * The in-memory wallet of the Rust library: simulated wallets and one simulated chain per network. Passed to
 * {@link Provider#create} it is used without Java callbacks. An unknown identifier given to a simulation
 * control throws {@link IllegalStateException} and leaves the backend unusable.
 */
public final class MemoryBackend implements WalletBackend, AutoCloseable {
    private final NativeHandle handle;

    private MemoryBackend(long handle) {
        this.handle = new NativeHandle(this, handle, Native::memoryFree);
    }

    @FunctionalInterface
    private interface Call<T> {
        T apply(long handle) throws PwcException;
    }

    private <T> T call(Call<T> f) throws PwcException {
        try {
            return f.apply(handle.get());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    private interface Run<T> {
        T apply(long handle);
    }

    private <T> T run(Run<T> f) {
        try {
            return f.apply(handle.get());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Per network: a software wallet with a P2WPKH and a P2PKH account and a watch-only wallet with one P2WPKH
     * account, each funded with confirmed coins; tips at height 1000.
     */
    public static MemoryBackend demo() {
        return new MemoryBackend(Native.memoryDemo());
    }

    long handle() {
        return handle.get();
    }

    // Simulation controls

    public String addWallet(String walletId, String name, String network, WalletKind kind) {
        return run(h -> Native.memoryAddWallet(h, walletId, name, network, kind));
    }

    public String addAccount(String walletId, String accountId, String name, ScriptType scriptType, boolean canSign) {
        return run(h -> Native.memoryAddAccount(h, walletId, accountId, name, scriptType, canSign));
    }

    public void setWalletState(String walletId, WalletState state) {
        run(h -> {
            Native.memorySetWalletState(h, walletId, state);
            return null;
        });
    }

    /** A payment of {@code value} satoshis to the account from a third party. */
    public String receive(String accountId, long value, int confirmations) {
        return run(h -> Native.memoryReceive(h, accountId, value, confirmations, false));
    }

    /** A coinbase output to the account. */
    public String receiveCoinbase(String accountId, long value, int confirmations) {
        return run(h -> Native.memoryReceive(h, accountId, value, confirmations, true));
    }

    /** A pending payment to the account that double-spends the input of the pending payment {@code txid}. */
    public String receiveConflicting(String accountId, String txid, long value) {
        return run(h -> Native.memoryReceiveConflicting(h, accountId, txid, value));
    }

    public void replace(String network, String txid, String by) {
        run(h -> {
            Native.memoryReplace(h, network, txid, by);
            return null;
        });
    }

    public void dropPending(String network, String txid) {
        run(h -> {
            Native.memoryDropPending(h, network, txid);
            return null;
        });
    }

    /** Mines {@code blocks} blocks; the first confirms every active pending transaction. */
    public void mine(String network, int blocks) {
        run(h -> {
            Native.memoryMine(h, network, blocks);
            return null;
        });
    }

    public void reorg(String network, int depth) {
        run(h -> {
            Native.memoryReorg(h, network, depth);
            return null;
        });
    }

    public void setOffline(String network, boolean offline) {
        run(h -> {
            Native.memorySetOffline(h, network, offline);
            return null;
        });
    }

    /** The wallet's fee estimate in sat/kvB; empty when it has none. */
    public void setFeeEstimate(String network, OptionalLong satPerKvb) {
        run(h -> {
            Native.memorySetFeeEstimate(h, network, satPerKvb.isPresent(), satPerKvb.orElse(0));
            return null;
        });
    }

    /** The User spends an output of the account from the wallet itself, paying {@code value} to a third party. */
    public String spendByUser(String accountId, OutPoint outpoint, long value) {
        return run(h -> Native.memorySpendByUser(h, accountId, outpoint, value));
    }

    public int tip(String network) {
        return run(h -> Native.memoryTip(h, network));
    }

    // WalletBackend

    @Override
    public List<WalletInfo> wallets() {
        return List.copyOf(run(Native::memoryWallets));
    }

    @Override
    public Optional<AccountInfo> account(String accountId) {
        return Optional.ofNullable(run(h -> Native.memoryAccount(h, accountId)));
    }

    @Override
    public Balance balance(String accountId, Set<OutPoint> reserved) throws PwcException {
        return call(h -> Native.memoryBalance(h, accountId, reserved));
    }

    @Override
    public AddressRecord receiveAddress(String accountId) throws PwcException {
        return call(h -> Native.memoryReceiveAddress(h, accountId));
    }

    @Override
    public AddressRecord createAddress(String accountId, String label) throws PwcException {
        return call(h -> Native.memoryCreateAddress(h, accountId, label));
    }

    @Override
    public List<AddressRecord> addresses(String accountId) {
        return List.copyOf(run(h -> Native.memoryAddresses(h, accountId)));
    }

    @Override
    public List<Utxo> utxos(String accountId, Set<OutPoint> reserved) throws PwcException {
        return List.copyOf(call(h -> Native.memoryUtxos(h, accountId, reserved)));
    }

    @Override
    public List<TransactionRecord> transactions(String accountId) throws PwcException {
        return List.copyOf(call(h -> Native.memoryTransactions(h, accountId)));
    }

    @Override
    public boolean ownsScript(String accountId, byte[] scriptPubKey) {
        return run(h -> Native.memoryOwnsScript(h, accountId, scriptPubKey));
    }

    @Override
    public boolean isUnspent(String accountId, OutPoint outpoint) {
        return run(h -> Native.memoryIsUnspent(h, accountId, outpoint));
    }

    @Override
    public OptionalLong feeEstimate(String network, long targetBlocks) {
        Long v = run(h -> Native.memoryFeeEstimate(h, network, targetBlocks));
        return v == null ? OptionalLong.empty() : OptionalLong.of(v);
    }

    @Override
    public CreatedPsbt createPsbt(String accountId, CreateRequest request, Set<OutPoint> excluded)
            throws PwcException {
        return call(h -> Native.memoryCreatePsbt(h, accountId, request, excluded));
    }

    @Override
    public Optional<TxOut> ownedOutput(String accountId, OutPoint outpoint) {
        return Optional.ofNullable(run(h -> Native.memoryOwnedOutput(h, accountId, outpoint)));
    }

    @Override
    public SignedPsbt signPsbt(String accountId, byte[] psbt, List<Integer> inputs) throws PwcException {
        return call(h -> Native.memorySignPsbt(h, accountId, psbt, inputs));
    }

    @Override
    public byte[] signMessage(String accountId, byte[] scriptPubKey, String message, MessageFormat format)
            throws PwcException {
        return call(h -> Native.memorySignMessage(h, accountId, scriptPubKey, message, format));
    }

    @Override
    public String broadcast(String network, byte[] transaction) throws PwcException {
        return call(h -> Native.memoryBroadcast(h, network, transaction));
    }

    @Override
    public void close() {
        handle.close();
    }
}
