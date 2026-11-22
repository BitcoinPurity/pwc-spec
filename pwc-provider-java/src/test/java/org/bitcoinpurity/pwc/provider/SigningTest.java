package org.bitcoinpurity.pwc.provider;

import static org.bitcoinpurity.pwc.provider.LocalClient.obj;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import java.nio.ByteBuffer;
import java.util.Base64;
import java.util.List;
import java.util.Set;
import java.util.function.Function;
import org.bitcoinj.core.Transaction;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class SigningTest {
    static final String WPKH = "a-regtest-wpkh";
    static final String[] SCOPES = {
        "balances:read", "utxos:read", "transactions:read", "psbt:create", "psbt:sign", "transactions:broadcast",
        "messages:sign"
    };

    ForwardingBackend java;

    Harness start(String backend) throws Exception {
        Function<MemoryBackend, WalletBackend> b =
                backend.equals("java") ? m -> java = new ForwardingBackend(m) : m -> m;
        return Harness.start(Config.defaults(), b);
    }

    @ParameterizedTest
    @ValueSource(strings = {"memory", "java"})
    void approvedPaymentIsFinalizedAndBroadcast(String backend) throws Exception {
        try (Harness h = start(backend)) {
            LocalClient c = h.client(SCOPES).client();
            String to = h.memory.receiveAddress("a-regtest-watch").address();
            JsonNode created = c.ok("psbt.create", obj(
                    "account_id", WPKH,
                    "outputs", List.of(obj("address", to, "amount_sats", "300000")),
                    "fee_policy", obj("type", "wallet_estimate", "target_blocks", 6),
                    "idempotency_key", "c")).get("result");
            JsonNode op = c.ok("psbt.sign", obj("account_id", WPKH, "psbt", created.get("psbt"), "idempotency_key", "s"));
            assertEquals("awaiting_approval", op.get("state").asText());

            PsbtSignPrompt prompt = h.nextPrompt(PsbtSignPrompt.class);
            PsbtSignRequest r = prompt.request();
            assertEquals(WPKH, r.accountId());
            assertEquals("regtest", r.network());
            assertEquals(List.of(0), r.inputsToSign());
            assertEquals(1, r.inputCount());
            assertEquals(created.get("fee_sats").asLong(), r.feeSats());
            assertEquals(2000, r.feeRateSatPerKvb());
            assertArrayEquals(Base64.getDecoder().decode(created.get("psbt").asText()), r.psbt());
            OutputSummary payment = r.outputs().stream().filter(o -> !o.toThisAccount()).findFirst().orElseThrow();
            assertEquals(to, payment.address());
            assertEquals(300_000, payment.amountSats());
            assertTrue(payment.scriptPubKey().startsWith("0014"));
            assertTrue(r.outputs().stream().anyMatch(OutputSummary::toThisAccount), "change");
            assertTrue(prompt.toString().contains("pay 300000 sat to " + to));
            prompt.approve();

            JsonNode done = c.waitFor(op.get("operation_id").asText());
            assertEquals("completed", done.get("state").asText(), done.toString());
            assertEquals("complete", done.get("result").get("signing_status").asText());
            JsonNode fin = c.ok("psbt.finalize", obj("psbt", done.get("result").get("psbt")));
            assertTrue(fin.get("complete").asBoolean());
            JsonNode sent = c.ok("transaction.broadcast", obj(
                    "account_id", WPKH, "raw_transaction", fin.get("raw_transaction"), "idempotency_key", "b"));
            assertEquals("completed", sent.get("state").asText(), sent.toString());
            Transaction tx = Transaction.read(ByteBuffer.wrap(
                    LocalClient.HEX.parseHex(fin.get("raw_transaction").asText())));
            assertEquals(tx.getTxId().toString(), sent.get("result").get("txid").asText());
            assertEquals("unconfirmed", c.ok("transaction.get", obj("account_id", WPKH, "txid", tx.getTxId().toString()))
                    .get("transaction").get("status").asText());

            String address = c.ok("utxo.list", obj("account_id", WPKH)).get("utxos").get(0).get("address").asText();
            String message = "Zahlung an Café – 支付 ✓  \n\t ";
            JsonNode sign = c.ok("message.sign", obj(
                    "account_id", WPKH, "address", address, "message", message, "format", "bip322-simple",
                    "idempotency_key", "m"));
            MessageSignPrompt mp = h.nextPrompt(MessageSignPrompt.class);
            assertEquals(new MessageSignRequest(mp.request().requester(), WPKH, "regtest", address, message,
                    MessageFormat.BIP322_SIMPLE), mp.request());
            mp.approve();
            JsonNode signed = c.waitFor(sign.get("operation_id").asText());
            JsonNode valid = c.ok("message.verify", obj(
                    "network", "regtest", "address", address, "message", message,
                    "signature", signed.get("result").get("signature"), "format", "bip322-simple"));
            assertTrue(valid.get("valid").asBoolean());

            if (java != null) {
                assertTrue(java.called.containsAll(Set.of(
                        "feeEstimate", "createPsbt", "ownsScript", "isUnspent", "ownedOutput", "signPsbt", "broadcast",
                        "signMessage")), java.called.toString());
            }
        }
    }

    @ParameterizedTest
    @ValueSource(strings = {"memory", "java"})
    void rejectedSignatures(String backend) throws Exception {
        try (Harness h = start(backend)) {
            LocalClient c = h.client(SCOPES).client();
            String to = h.memory.receiveAddress("a-regtest-watch").address();
            JsonNode created = c.ok("psbt.create", obj(
                    "account_id", WPKH,
                    "outputs", List.of(obj("address", to, "amount_sats", "1000")),
                    "fee_policy", obj("type", "rate", "sat_per_kvb", "1000"),
                    "idempotency_key", "c")).get("result");
            JsonNode op = c.ok("psbt.sign", obj("account_id", WPKH, "psbt", created.get("psbt"), "idempotency_key", "s"));
            h.nextPrompt(PsbtSignPrompt.class).reject();
            assertEquals("rejected", c.waitFor(op.get("operation_id").asText()).get("state").asText());

            String address = c.ok("utxo.list", obj("account_id", WPKH)).get("utxos").get(0).get("address").asText();
            JsonNode sign = c.ok("message.sign", obj(
                    "account_id", WPKH, "address", address, "message", "m", "format", "bip322-simple",
                    "idempotency_key", "m"));
            MessageSignPrompt mp = h.nextPrompt(MessageSignPrompt.class);
            assertFalse(mp.isWithdrawn());
            mp.reject();
            assertEquals("rejected", c.waitFor(sign.get("operation_id").asText()).get("state").asText());
        }
    }
}
