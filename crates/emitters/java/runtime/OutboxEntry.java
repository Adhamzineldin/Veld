package dev.veld.runtime;

/**
 * One message the application's store holds until the relay has published it.
 *
 * @param id the application's own, opaque identity of the entry
 * @param topic the topic the message goes to
 * @param key the key text of the message, or {@code null} when the channel has no key
 * @param bytes the bytes of the envelope
 */
public record OutboxEntry(String id, String topic, String key, byte[] bytes) {}
