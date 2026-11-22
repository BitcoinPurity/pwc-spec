package org.bitcoinpurity.pwc.provider;

import java.lang.ref.Cleaner;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.LongConsumer;

/** A pointer to native memory, freed exactly once: by {@link #close()} or when its owner is unreachable. */
final class NativeHandle {
    private static final Cleaner CLEANER = Cleaner.create();

    private record Free(AtomicLong pointer, LongConsumer free) implements Runnable {
        @Override
        public void run() {
            long p = pointer.getAndSet(0);
            if (p != 0) {
                free.accept(p);
            }
        }
    }

    private final AtomicLong pointer;
    private final Cleaner.Cleanable cleanable;

    NativeHandle(Object owner, long pointer, LongConsumer free) {
        this.pointer = new AtomicLong(pointer);
        this.cleanable = CLEANER.register(owner, new Free(this.pointer, free));
    }

    long get() {
        long p = pointer.get();
        if (p == 0) {
            throw new IllegalStateException("closed");
        }
        return p;
    }

    void close() {
        cleanable.clean();
    }
}
