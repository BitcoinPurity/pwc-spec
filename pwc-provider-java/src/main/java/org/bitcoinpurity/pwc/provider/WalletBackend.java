package org.bitcoinpurity.pwc.provider;

import java.util.List;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.Set;

/**
 * The wallet behind the Provider: inventory, chain data, keys and broadcast. The methods are those of the Rust
 * {@code WalletBackend} trait and are called from the Provider's threads, never concurrently with each other.
 * Account data methods are called only for existing accounts of readable wallets.
 *
 * <p>A method that has not returned holds up the other Core requests, which all need the backend, but not the
 * handshake, pairing, the answers to prompts, {@link Provider#notify} or the host's own calls on the
 * {@link Provider}. A method may call {@link Provider#notify}.
 *
 * <p>A {@link PwcException} thrown by a method that declares it becomes the request's error; any other
 * exception gives {@code INTERNAL_ERROR}. A method that declares none is treated as returning nothing (an
 * empty list, an empty optional or {@code false}) when it throws, and the exception is printed.
 */
public interface WalletBackend {
    List<WalletInfo> wallets();

    Optional<AccountInfo> account(String accountId);

    /** {@code reserved} outpoints are reported as locked (Core §8.3). */
    Balance balance(String accountId, Set<OutPoint> reserved) throws PwcException;

    /** The current receive address; repeated calls may return the same one. */
    AddressRecord receiveAddress(String accountId) throws PwcException;

    /** {@code label} is nullable. */
    AddressRecord createAddress(String accountId, String label) throws PwcException;

    List<AddressRecord> addresses(String accountId);

    List<Utxo> utxos(String accountId, Set<OutPoint> reserved) throws PwcException;

    /** Newest first. */
    List<TransactionRecord> transactions(String accountId) throws PwcException;

    boolean ownsScript(String accountId, byte[] scriptPubKey);

    /** Whether the outpoint is an effective unspent output of the account. */
    boolean isUnspent(String accountId, OutPoint outpoint);

    /** The wallet's fee estimate in sat/kvB, empty if it cannot compute one now. */
    OptionalLong feeEstimate(String network, long targetBlocks);

    /** Selects coins and builds an unsigned PSBT; {@code excluded} outpoints are not selected. */
    CreatedPsbt createPsbt(String accountId, CreateRequest request, Set<OutPoint> excluded)
            throws PwcException;

    /** The account's output at {@code outpoint} on the account's network, if the account owns it. */
    Optional<TxOut> ownedOutput(String accountId, OutPoint outpoint);

    /**
     * Adds the account's {@code SIGHASH_ALL} signatures to the given inputs, whose {@code witness_utxo} is set.
     * The returned PSBT replaces the given one.
     */
    SignedPsbt signPsbt(String accountId, byte[] psbt, List<Integer> inputs) throws PwcException;

    /** Signs the exact UTF-8 bytes of {@code message} with the account key of {@code scriptPubKey}. */
    byte[] signMessage(String accountId, byte[] scriptPubKey, String message, MessageFormat format)
            throws PwcException;

    /**
     * Submits a transaction in consensus serialization and returns its txid; {@code TRANSACTION_REJECTED} or
     * {@code NETWORK_UNAVAILABLE} on failure.
     */
    String broadcast(String network, byte[] transaction) throws PwcException;

    default Optional<WalletInfo> wallet(String walletId) {
        return wallets().stream().filter(w -> w.walletId().equals(walletId)).findFirst();
    }
}
