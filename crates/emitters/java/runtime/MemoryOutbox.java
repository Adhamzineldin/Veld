package dev.veld.runtime;

import java.util.ArrayList;
import java.util.List;

/** An outbox in memory, for tests: the ids are {@code "1"}, {@code "2"}, ... in the order of the saves. */
public final class MemoryOutbox implements Outbox {
  private final List<OutboxEntry> entries = new ArrayList<>();
  private long next = 1;

  /** Creates an empty outbox. */
  public MemoryOutbox() {}

  @Override
  public synchronized void save(String topic, String key, byte[] bytes) {
    entries.add(new OutboxEntry(Long.toString(next), topic, key, bytes));
    next += 1;
  }

  @Override
  public synchronized List<OutboxEntry> pending(int limit) {
    return List.copyOf(entries.subList(0, Math.max(0, Math.min(limit, entries.size()))));
  }

  @Override
  public synchronized void done(OutboxEntry entry) {
    entries.removeIf(e -> e.id().equals(entry.id()));
  }

  /** The number of entries still pending. */
  public synchronized int size() {
    return entries.size();
  }
}
