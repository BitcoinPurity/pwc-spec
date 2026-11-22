package org.bitcoinpurity.pwc.provider;

import java.time.Duration;

/** Timeouts and limits of a Provider. {@link #defaults()} gives the values of the Rust library. */
public record Config(
        String providerName,
        String providerVersion,
        Limits limits,
        Duration helloTimeout,
        Duration authTimeout,
        Duration pairingTimeout,
        Duration approvalTimeout,
        Duration reservationTtl,
        RateLimit pairingLimit,
        RateLimit authFailureLimit,
        Duration authFailureDelay,
        RateLimit promptLimit,
        int eventBuffer) {

    public static Config defaults() {
        return Native.defaultConfig();
    }

    public Builder toBuilder() {
        return new Builder(this);
    }

    public static final class Builder {
        private String providerName;
        private String providerVersion;
        private Limits limits;
        private Duration helloTimeout;
        private Duration authTimeout;
        private Duration pairingTimeout;
        private Duration approvalTimeout;
        private Duration reservationTtl;
        private RateLimit pairingLimit;
        private RateLimit authFailureLimit;
        private Duration authFailureDelay;
        private RateLimit promptLimit;
        private int eventBuffer;

        private Builder(Config c) {
            providerName = c.providerName;
            providerVersion = c.providerVersion;
            limits = c.limits;
            helloTimeout = c.helloTimeout;
            authTimeout = c.authTimeout;
            pairingTimeout = c.pairingTimeout;
            approvalTimeout = c.approvalTimeout;
            reservationTtl = c.reservationTtl;
            pairingLimit = c.pairingLimit;
            authFailureLimit = c.authFailureLimit;
            authFailureDelay = c.authFailureDelay;
            promptLimit = c.promptLimit;
            eventBuffer = c.eventBuffer;
        }

        public Builder providerName(String v) {
            providerName = v;
            return this;
        }

        public Builder providerVersion(String v) {
            providerVersion = v;
            return this;
        }

        public Builder limits(Limits v) {
            limits = v;
            return this;
        }

        public Builder helloTimeout(Duration v) {
            helloTimeout = v;
            return this;
        }

        public Builder authTimeout(Duration v) {
            authTimeout = v;
            return this;
        }

        public Builder pairingTimeout(Duration v) {
            pairingTimeout = v;
            return this;
        }

        public Builder approvalTimeout(Duration v) {
            approvalTimeout = v;
            return this;
        }

        public Builder reservationTtl(Duration v) {
            reservationTtl = v;
            return this;
        }

        public Builder pairingLimit(RateLimit v) {
            pairingLimit = v;
            return this;
        }

        public Builder authFailureLimit(RateLimit v) {
            authFailureLimit = v;
            return this;
        }

        public Builder authFailureDelay(Duration v) {
            authFailureDelay = v;
            return this;
        }

        public Builder promptLimit(RateLimit v) {
            promptLimit = v;
            return this;
        }

        public Builder eventBuffer(int v) {
            eventBuffer = v;
            return this;
        }

        public Config build() {
            return new Config(
                    providerName,
                    providerVersion,
                    limits,
                    helloTimeout,
                    authTimeout,
                    pairingTimeout,
                    approvalTimeout,
                    reservationTtl,
                    pairingLimit,
                    authFailureLimit,
                    authFailureDelay,
                    promptLimit,
                    eventBuffer);
        }
    }
}
