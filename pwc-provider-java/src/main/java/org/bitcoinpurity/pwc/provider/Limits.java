package org.bitcoinpurity.pwc.provider;

/** {@code Capabilities.limits} (Core §4.1); {@code highFeeRate} is in sat/kvB. */
public record Limits(
        long maxPageSize,
        long maxPsbtBytes,
        long maxCombinePsbts,
        long maxRawTransactionBytes,
        long maxMessageBytes,
        long maxConcurrentOperations,
        long operationRetentionSeconds,
        long highFeeRate) {

    /** The defaults of the Rust library. */
    public static Limits defaults() {
        return Config.defaults().limits();
    }

    public Builder toBuilder() {
        return new Builder(this);
    }

    public static final class Builder {
        private long maxPageSize;
        private long maxPsbtBytes;
        private long maxCombinePsbts;
        private long maxRawTransactionBytes;
        private long maxMessageBytes;
        private long maxConcurrentOperations;
        private long operationRetentionSeconds;
        private long highFeeRate;

        private Builder(Limits l) {
            maxPageSize = l.maxPageSize;
            maxPsbtBytes = l.maxPsbtBytes;
            maxCombinePsbts = l.maxCombinePsbts;
            maxRawTransactionBytes = l.maxRawTransactionBytes;
            maxMessageBytes = l.maxMessageBytes;
            maxConcurrentOperations = l.maxConcurrentOperations;
            operationRetentionSeconds = l.operationRetentionSeconds;
            highFeeRate = l.highFeeRate;
        }

        public Builder maxPageSize(long v) {
            maxPageSize = v;
            return this;
        }

        public Builder maxPsbtBytes(long v) {
            maxPsbtBytes = v;
            return this;
        }

        public Builder maxCombinePsbts(long v) {
            maxCombinePsbts = v;
            return this;
        }

        public Builder maxRawTransactionBytes(long v) {
            maxRawTransactionBytes = v;
            return this;
        }

        public Builder maxMessageBytes(long v) {
            maxMessageBytes = v;
            return this;
        }

        public Builder maxConcurrentOperations(long v) {
            maxConcurrentOperations = v;
            return this;
        }

        public Builder operationRetentionSeconds(long v) {
            operationRetentionSeconds = v;
            return this;
        }

        public Builder highFeeRate(long v) {
            highFeeRate = v;
            return this;
        }

        public Limits build() {
            return new Limits(
                    maxPageSize,
                    maxPsbtBytes,
                    maxCombinePsbts,
                    maxRawTransactionBytes,
                    maxMessageBytes,
                    maxConcurrentOperations,
                    operationRetentionSeconds,
                    highFeeRate);
        }
    }
}
