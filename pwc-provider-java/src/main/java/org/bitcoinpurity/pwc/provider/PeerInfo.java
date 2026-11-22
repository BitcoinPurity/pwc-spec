package org.bitcoinpurity.pwc.provider;

import java.nio.file.Path;

/** A connected process as reported by the operating system; either field is null when unknown. */
public record PeerInfo(Integer pid, Path executable) {}
