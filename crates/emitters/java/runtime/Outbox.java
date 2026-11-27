package dev.veld.runtime;

import java.util.List;

/**
 * The application's outbox, over its own store (spec/events.md section 11.1): {@code save} MUST run
 * in the transaction that changes the state, so that the state and its event stand or fall
 * together. {@code pending} lists the entries not yet done, in the order they were saved (at most
 * {@code limit}); {@code done} removes one. The server cannot check that {@code save} shares the
 * application's transaction.
 */
public interface Outbox {
  /** Writes one message: the topic, the key text of the message (or {@code null}) and its bytes. */
  void save(String topic, String key, byte[] bytes) throws Exception;

  /** The oldest entries not yet done, in the order they were saved, at most {@code limit}. */
  List<OutboxEntry> pending(int limit) throws Exception;

  /** Marks one entry as published: it is not pending any more. */
  void done(OutboxEntry entry) throws Exception;
}
