package org.bitcoinpurity.pwc.provider;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.time.Duration;
import org.junit.jupiter.api.Test;

class ConfigTest {
    @Test
    void defaultsAreThoseOfTheRustLibrary() {
        Config c = Config.defaults();
        assertEquals("PWC Provider", c.providerName());
        assertEquals("0.1.0", c.providerVersion());
        assertEquals(Duration.ofSeconds(10), c.helloTimeout());
        assertEquals(Duration.ofSeconds(60), c.authTimeout());
        assertEquals(Duration.ofSeconds(300), c.pairingTimeout());
        assertEquals(Duration.ofSeconds(300), c.approvalTimeout());
        assertEquals(Duration.ofSeconds(600), c.reservationTtl());
        assertEquals(new RateLimit(10, Duration.ofSeconds(60)), c.pairingLimit());
        assertEquals(new RateLimit(10, Duration.ofSeconds(60)), c.authFailureLimit());
        assertEquals(Duration.ofMillis(100), c.authFailureDelay());
        assertEquals(new RateLimit(10, Duration.ofSeconds(60)), c.promptLimit());
        assertEquals(1024, c.eventBuffer());
        assertEquals(Limits.defaults(), c.limits());
    }

    @Test
    void limitDefaultsAreThoseOfTheApiContract() {
        Limits l = Limits.defaults();
        assertEquals(100, l.maxPageSize());
        assertEquals(100_000, l.maxPsbtBytes());
        assertEquals(5, l.maxCombinePsbts());
        assertEquals(400_000, l.maxRawTransactionBytes());
        assertEquals(65_536, l.maxMessageBytes());
        assertEquals(8, l.maxConcurrentOperations());
        assertEquals(86_400, l.operationRetentionSeconds());
        assertEquals(500_000, l.highFeeRate());
    }

    @Test
    void builderChangesOnlyTheGivenFields() {
        Config c = Config.defaults().toBuilder().providerName("Host wallet").eventBuffer(8).build();
        assertEquals("Host wallet", c.providerName());
        assertEquals(8, c.eventBuffer());
        assertEquals(Config.defaults().toBuilder().providerName("Host wallet").eventBuffer(8).build(), c);
        assertEquals(Config.defaults().approvalTimeout(), c.approvalTimeout());

        Limits l = Limits.defaults().toBuilder().maxPageSize(10).build();
        assertEquals(10, l.maxPageSize());
        assertEquals(Limits.defaults().highFeeRate(), l.highFeeRate());
    }
}
