package dev.veld.runtime;

/**
 * An answer an {@link IdempotencyStore} keeps: what is sent again when the key comes back.
 *
 * @param status the HTTP status
 * @param body the bytes that were sent; empty for an answer without a body
 * @param contentType the {@code Content-Type}, or {@code null}
 * @param etag the {@code ETag}, or {@code null}
 * @param location the {@code Location}, or {@code null}
 */
public record StoredAnswer(int status, byte[] body, String contentType, String etag, String location) {}
