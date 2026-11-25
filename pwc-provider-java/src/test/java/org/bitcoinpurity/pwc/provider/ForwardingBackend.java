package org.bitcoinpurity.pwc.provider;

import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/** A Java backend that forwards to the in-memory one, records each method called and can be made to fail. */
final class ForwardingBackend implements WalletBackend {
    static final Set<String> METHODS = Set.of(
            "wallets", "account", "balance", "receiveAddress", "createAddress", "addresses", "utxos", "transactions",
            "ownsScript", "isUnspent", "feeEstimate", "createPsbt", "ownedOutput", "signPsbt", "signMessage",
            "broadcast");

    final MemoryBackend memory;
    final Set<String> called = ConcurrentHashMap.newKeySet();
    /** A method's name mapped to the exception it throws: a {@link PwcException} or a RuntimeException. */
    final Map<String, Exception> failures = new ConcurrentHashMap<>();

    ForwardingBackend(MemoryBackend memory) {
        this.memory = memory;
    }

    private void enter(String method) throws PwcException {
        called.add(method);
        Exception e = failures.get(method);
        if (e instanceof PwcException p) {
            throw p;
        }
        if (e != null) {
            throw (RuntimeException) e;
        }
    }

    private void enterUnchecked(String method) {
        called.add(method);
        Exception e = failures.get(method);
        if (e != null) {
            throw (RuntimeException) e;
        }
    }

    @Override
    public List<WalletInfo> wallets() {
        enterUnchecked("wallets");
        return memory.wallets();
    }

    @Override
    public Optional<AccountInfo> account(String accountId) {
        enterUnchecked("account");
        return memory.account(accountId);
    }

    @Override
    public Balance balance(String accountId, Set<OutPoint> reserved) throws PwcException {
        enter("balance");
        return memory.balance(accountId, reserved);
    }

    @Override
    public AddressRecord receiveAddress(String accountId) throws PwcException {
        enter("receiveAddress");
        return memory.receiveAddress(accountId);
    }

    @Override
    public AddressRecord createAddress(String accountId, String label) throws PwcException {
        enter("createAddress");
        return memory.createAddress(accountId, label);
    }

    @Override
    public List<AddressRecord> addresses(String accountId) {
        enterUnchecked("addresses");
        return memory.addresses(accountId);
    }

    @Override
    public List<Utxo> utxos(String accountId, Set<OutPoint> reserved) throws PwcException {
        enter("utxos");
        return memory.utxos(accountId, reserved);
    }

    @Override
    public List<TransactionRecord> transactions(String accountId) throws PwcException {
        enter("transactions");
        return memory.transactions(accountId);
    }

    @Override
    public boolean ownsScript(String accountId, byte[] script) {
        enterUnchecked("ownsScript");
        return memory.ownsScript(accountId, script);
    }

    @Override
    public boolean isUnspent(String accountId, OutPoint outpoint) {
        enterUnchecked("isUnspent");
        return memory.isUnspent(accountId, outpoint);
    }

    @Override
    public OptionalLong feeEstimate(String network, long targetBlocks) {
        enterUnchecked("feeEstimate");
        return memory.feeEstimate(network, targetBlocks);
    }

    @Override
    public CreatedPsbt createPsbt(String accountId, CreateRequest request, Set<OutPoint> excluded)
            throws PwcException {
        enter("createPsbt");
        return memory.createPsbt(accountId, request, excluded);
    }

    @Override
    public Optional<TxOut> ownedOutput(String accountId, OutPoint outpoint) {
        enterUnchecked("ownedOutput");
        return memory.ownedOutput(accountId, outpoint);
    }

    @Override
    public SignedPsbt signPsbt(String accountId, byte[] psbt, List<Integer> inputs) throws PwcException {
        enter("signPsbt");
        return memory.signPsbt(accountId, psbt, inputs);
    }

    @Override
    public byte[] signMessage(String accountId, byte[] script, String message, MessageFormat format)
            throws PwcException {
        enter("signMessage");
        return memory.signMessage(accountId, script, message, format);
    }

    @Override
    public String broadcast(String network, byte[] tx) throws PwcException {
        enter("broadcast");
        return memory.broadcast(network, tx);
    }
}
