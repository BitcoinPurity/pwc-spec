package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.ByteBuffer;
import java.util.List;
import java.util.OptionalLong;
import java.util.Set;
import org.bitcoinj.base.Coin;
import org.bitcoinj.base.Sha256Hash;
import org.bitcoinj.core.Transaction;
import org.bitcoinj.core.TransactionOutPoint;
import org.bitcoinj.core.TransactionOutput;
import org.junit.jupiter.api.Test;

class MemoryBackendTest {
    static final String WPKH = "a-regtest-wpkh";
    static final String PKH = "a-regtest-pkh";

    @Test
    void inventory() {
        try (MemoryBackend m = MemoryBackend.demo()) {
            List<WalletInfo> wallets = m.wallets();
            assertEquals(6, wallets.size());
            WalletInfo main = m.wallet("w-regtest-main").orElseThrow();
            assertEquals(new WalletInfo("w-regtest-main", "Main wallet", "regtest", WalletKind.SOFTWARE,
                    WalletState.READY, List.of(WPKH, PKH)), main);
            assertEquals(WalletKind.WATCH_ONLY, m.wallet("w-regtest-watch").orElseThrow().kind());

            AccountInfo a = m.account(WPKH).orElseThrow();
            assertEquals("w-regtest-main", a.walletId());
            assertEquals("Savings", a.displayName());
            assertTrue(a.canSign());
            assertEquals(ScriptType.P2WPKH, a.scriptType());
            assertTrue(a.methods().contains("psbt.sign"));
            assertFalse(m.account("a-regtest-watch").orElseThrow().methods().contains("psbt.sign"));
            assertEquals(ScriptType.P2PKH, m.account(PKH).orElseThrow().scriptType());
            assertTrue(m.account("no-such-account").isEmpty());
        }
    }

    @Test
    void balancesAddressesUtxosAndTransactions() throws Exception {
        try (MemoryBackend m = MemoryBackend.demo()) {
            Balance b = m.balance(WPKH, Set.of());
            assertEquals(WPKH, b.accountId());
            assertEquals("regtest", b.network());
            assertEquals(new BalanceComponents(1_500_000, 0, 0, 0, 0, 1_500_000, 0, 0), b.components());
            assertEquals(new SyncStatus(SyncState.SYNCED, 1000), b.sync());

            AddressRecord receive = m.receiveAddress(WPKH);
            assertEquals(AddressRole.RECEIVE, receive.role());
            assertTrue(receive.address().startsWith("bcrt1q"), receive.address());
            AddressRecord created = m.createAddress(WPKH, "Invoice 7");
            assertEquals("Invoice 7", created.label());
            assertEquals(AddressState.UNUSED, created.state());
            assertTrue(m.addresses(WPKH).contains(created));
            assertNull(m.createAddress(WPKH, null).label());

            List<Utxo> utxos = m.utxos(WPKH, Set.of());
            assertEquals(2, utxos.size());
            Utxo u = utxos.stream().filter(x -> x.valueSats() == 1_000_000).findFirst().orElseThrow();
            assertEquals(6, u.confirmations());
            assertEquals(995, u.blockHeight());
            assertTrue(u.spendable());
            assertEquals(List.of(), u.restrictions());
            assertFalse(u.isChange());
            assertTrue(m.ownsScript(WPKH, u.scriptPubKey()));
            assertFalse(m.ownsScript(PKH, u.scriptPubKey()));
            TransactionOutPoint outpoint = new TransactionOutPoint(u.vout(), u.txid());
            assertTrue(m.isUnspent(WPKH, outpoint));
            TransactionOutput owned = m.ownedOutput(WPKH, outpoint).orElseThrow();
            assertEquals(Coin.valueOf(1_000_000), owned.getValue());
            assertArrayEquals(u.scriptPubKey().program(), owned.getScriptBytes());

            Utxo reserved = m.utxos(WPKH, Set.of(outpoint)).stream()
                    .filter(x -> x.txid().equals(u.txid()) && x.vout() == u.vout())
                    .findFirst()
                    .orElseThrow();
            assertEquals(List.of(UtxoRestriction.LOCKED), reserved.restrictions());
            assertEquals(1_000_000, m.balance(WPKH, Set.of(outpoint)).components().lockedSats());

            List<TransactionRecord> txs = m.transactions(WPKH);
            assertEquals(2, txs.size());
            TransactionRecord t = txs.stream().filter(x -> x.txid().equals(u.txid())).findFirst().orElseThrow();
            assertEquals(TransactionStatus.CONFIRMED, t.status());
            assertEquals(1_000_000, t.netChangeSats());
            assertNull(t.replacedByTxid());
            assertEquals(OptionalLong.of(2000), m.feeEstimate("regtest", 6));
        }
    }

    @Test
    void simulationControls() throws Exception {
        try (MemoryBackend m = MemoryBackend.demo()) {
            Sha256Hash pending = m.receive(WPKH, 40_000, 0);
            assertEquals(40_000, m.balance(WPKH, Set.of()).components().pendingExternalSats());
            assertEquals(TransactionStatus.UNCONFIRMED, status(m, pending));

            Sha256Hash conflicting = m.receiveConflicting(WPKH, pending, 30_000);
            m.replace("regtest", pending, conflicting);
            TransactionRecord replaced = record(m, pending);
            assertEquals(TransactionStatus.REPLACED, replaced.status());
            assertEquals(conflicting, replaced.replacedByTxid());
            m.dropPending("regtest", conflicting);
            assertEquals(TransactionStatus.DROPPED, status(m, conflicting));

            Sha256Hash next = m.receive(WPKH, 25_000, 0);
            m.mine("regtest", 2);
            assertEquals(1002, m.tip("regtest"));
            assertEquals(2, record(m, next).confirmations());
            m.reorg("regtest", 2);
            assertEquals(1000, m.tip("regtest"));
            assertEquals(TransactionStatus.UNCONFIRMED, status(m, next));

            m.receiveCoinbase(PKH, 5_000_000, 10);
            assertEquals(5_000_000, m.balance(PKH, Set.of()).components().immatureSats());

            Utxo u = m.utxos(WPKH, Set.of()).get(0);
            TransactionOutPoint outpoint = new TransactionOutPoint(u.vout(), u.txid());
            m.spendByUser(WPKH, outpoint, u.valueSats() - 1000);
            assertFalse(m.isUnspent(WPKH, outpoint));

            m.setWalletState("w-regtest-main", WalletState.LOCKED);
            assertEquals(WalletState.LOCKED, m.wallet("w-regtest-main").orElseThrow().state());
            m.setOffline("regtest", true);
            assertEquals(SyncState.OFFLINE, m.balance(WPKH, Set.of()).sync().state());
            m.setFeeEstimate("regtest", OptionalLong.empty());
            assertEquals(OptionalLong.empty(), m.feeEstimate("regtest", 6));

            String w = m.addWallet("w-extra", "Extra", "regtest", WalletKind.HARDWARE);
            String a = m.addAccount(w, "a-extra", "Extra account", ScriptType.P2WPKH, false);
            assertEquals(List.of(a), m.wallet(w).orElseThrow().accountIds());
            assertFalse(m.account(a).orElseThrow().canSign());
        }
    }

    @Test
    void psbtsMessagesAndBroadcast() throws Exception {
        try (MemoryBackend m = MemoryBackend.demo()) {
            TransactionOutput pay = new TransactionOutput(
                    null, Coin.valueOf(100_000), m.utxos(PKH, Set.of()).get(0).scriptPubKey().program());
            CreatedPsbt created = m.createPsbt(WPKH, new CreateRequest(List.of(pay), 2000, null, true), Set.of());
            assertTrue(created.fee() > 0 && created.vsize() > 0);
            assertEquals(1, created.inputs().size());
            assertEquals(1, created.changeOutputIndexes().size());

            List<Integer> all = List.of(0);
            SignedPsbt signed = m.signPsbt(WPKH, created.psbt(), all);
            assertEquals(all, signed.signedInputs());
            assertNotEquals(created.psbt().length, signed.psbt().length);

            PwcException policy = assertThrows(PwcException.class,
                    () -> m.createPsbt(WPKH, new CreateRequest(List.of(pay), 10, null, true), Set.of()));
            assertEquals(ErrorCode.POLICY_VIOLATION, policy.code());

            byte[] signature = m.signMessage(WPKH, m.utxos(WPKH, Set.of()).get(0).scriptPubKey(), "hello",
                    MessageFormat.BIP322_SIMPLE);
            assertTrue(signature.length > 64);
            PwcException full = assertThrows(PwcException.class, () -> m.signMessage(
                    WPKH, m.utxos(WPKH, Set.of()).get(0).scriptPubKey(), "hello", MessageFormat.BIP322_FULL));
            assertEquals(ErrorCode.UNSUPPORTED_OPERATION, full.code());

            Transaction unsigned = Transaction.read(ByteBuffer.wrap(unsignedTx(created.psbt())));
            PwcException rejected = assertThrows(PwcException.class, () -> m.broadcast("regtest", unsigned));
            assertEquals(ErrorCode.TRANSACTION_REJECTED, rejected.code());
            m.setOffline("regtest", true);
            assertEquals(ErrorCode.NETWORK_UNAVAILABLE,
                    assertThrows(PwcException.class, () -> m.broadcast("regtest", unsigned)).code());
        }
    }

    /** The unsigned transaction of a version 0 PSBT: the value of its first global key-value pair. */
    static byte[] unsignedTx(byte[] psbt) {
        ByteBuffer b = ByteBuffer.wrap(psbt, 5, psbt.length - 5);
        int keyLength = b.get() & 0xff;
        b.position(b.position() + keyLength);
        int first = b.get() & 0xff;
        int length = first < 0xfd ? first : first == 0xfd ? (b.get() & 0xff) | (b.get() & 0xff) << 8 : -1;
        byte[] tx = new byte[length];
        b.get(tx);
        return tx;
    }

    private static TransactionRecord record(MemoryBackend m, Sha256Hash txid) throws PwcException {
        return m.transactions(WPKH).stream().filter(t -> t.txid().equals(txid)).findFirst().orElseThrow();
    }

    private static TransactionStatus status(MemoryBackend m, Sha256Hash txid) throws PwcException {
        return record(m, txid).status();
    }
}
