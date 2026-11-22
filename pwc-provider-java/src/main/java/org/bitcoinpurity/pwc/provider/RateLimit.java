package org.bitcoinpurity.pwc.provider;

import java.time.Duration;

/** At most {@code max} events per {@code window}. */
public record RateLimit(int max, Duration window) {}
