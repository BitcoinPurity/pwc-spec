package org.bitcoinpurity.pwc.provider;

import static org.bitcoinpurity.pwc.provider.LocalClient.obj;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;
import org.junit.jupiter.api.Test;

class PermissionsTest {
    @Test
    void permissionPromptCarriesTheRequestAndGrantCreatesAGrant() throws Exception {
        try (Harness h = Harness.start()) {
            Harness.Paired p = h.pair();
            Instant until = Instant.now().plus(1, ChronoUnit.DAYS).truncatedTo(ChronoUnit.MILLIS);
            JsonNode op = p.client().ok("session.requestPermissions", obj(
                    "scopes", List.of("wallets:read", "balances:read"),
                    "network", "regtest",
                    "wallet_ids", List.of("w-regtest-main", "no-such-wallet"),
                    "expires_at", until.toString(),
                    "constraints", obj("max_amount_sats_per_request", "50000"),
                    "idempotency_key", "k-1"));
            PermissionPrompt prompt = h.nextPrompt(PermissionPrompt.class);
            PermissionRequest r = prompt.request();
            assertEquals(p.clientId(), r.requester().clientId());
            assertEquals("Test client", r.requester().claimedName());
            assertEquals(List.of("wallets:read", "balances:read"), r.scopes());
            assertEquals("regtest", r.network());
            assertEquals(List.of("w-regtest-main"), r.walletIds(), "unknown wallets removed");
            assertEquals(List.of(), r.accountIds());
            assertEquals(until, r.expiresAt());
            assertEquals(new GrantConstraints(50_000L, null), r.constraints());
            assertTrue(prompt.toString().contains("Permission request from"));
            prompt.grant(h.asRequested(r));
            assertThrows(IllegalStateException.class, () -> prompt.grant(h.asRequested(r)));

            JsonNode done = p.client().waitFor(op.get("operation_id").asText());
            assertEquals("completed", done.get("state").asText());
            List<Grant> grants = h.provider.grants();
            assertEquals(1, grants.size());
            Grant g = grants.get(0);
            assertEquals(p.clientId(), g.clientId());
            assertEquals("regtest", g.network());
            assertEquals(List.of("wallets:read", "balances:read"), g.scopes());
            assertEquals(List.of("w-regtest-main"), g.walletIds());
            assertEquals(until, g.expiresAt());
            assertEquals(GrantStatus.ACTIVE, g.status());
            assertEquals(50_000L, g.constraints().maxAmountSatsPerRequest());
            assertTrue(g.createdAt().isBefore(Instant.now().plusSeconds(1)));

            JsonNode wallets = p.client().ok("wallet.list", obj()).get("wallets");
            assertEquals(1, wallets.size());
            assertEquals("w-regtest-main", wallets.get(0).get("wallet_id").asText());

            assertTrue(h.provider.revokeGrant(g.grantId()));
            assertFalse(h.provider.revokeGrant("no-such-grant"));
            assertEquals(GrantStatus.REVOKED, h.provider.grants().get(0).status());
            assertEquals("PERMISSION_REVOKED", p.client().fails("wallet.list", obj()));
        }
    }

    @Test
    void rejectedPermissionRequest() throws Exception {
        try (Harness h = Harness.start()) {
            Harness.Paired p = h.pair();
            JsonNode op = p.client().ok(
                    "session.requestPermissions", obj("scopes", List.of("wallets:read"), "idempotency_key", "k-1"));
            PermissionPrompt prompt = h.nextPrompt(PermissionPrompt.class);
            assertNull(prompt.request().network());
            assertNull(prompt.request().expiresAt());
            assertNull(prompt.request().constraints());
            prompt.reject();
            assertEquals("rejected", p.client().waitFor(op.get("operation_id").asText()).get("state").asText());
            assertTrue(h.provider.grants().isEmpty());
        }
    }

    @Test
    void grantChoiceIsValidated() throws Exception {
        try (Harness h = Harness.start()) {
            Harness.Paired p = h.pair();
            p.client().ok("session.requestPermissions", obj("scopes", List.of("wallets:read"), "idempotency_key", "k-1"));
            PermissionPrompt prompt = h.nextPrompt(PermissionPrompt.class);
            GrantChoice bad = new GrantChoice("Not A Network", List.of("wallets:read"), List.of(), List.of(), null, null);
            assertThrows(IllegalArgumentException.class, () -> prompt.grant(bad));
            prompt.grant(h.asRequested(prompt.request()));
        }
    }
}
