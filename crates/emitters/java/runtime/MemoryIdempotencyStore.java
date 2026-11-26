package dev.veld.runtime;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * An {@link IdempotencyStore} in memory, for tests. It keeps every entry for as long as it lives
 * and is safe to share between threads.
 */
public final class MemoryIdempotencyStore implements IdempotencyStore {

  /** One key: the fingerprint it was started with, and the answer once there is one. */
  private record Entry(String fingerprint, StoredAnswer answer) {}

  private final Map<List<String>, Entry> entries = new HashMap<>();

  /** Creates an empty store. */
  public MemoryIdempotencyStore() {}

  @Override
  public synchronized Begin begin(String scope, String key, String fingerprint) {
    List<String> id = List.of(scope, key);
    Entry found = entries.get(id);
    if (found == null) {
      entries.put(id, new Entry(fingerprint, null));
      return new Begin.Started();
    }
    if (!found.fingerprint().equals(fingerprint)) {
      return new Begin.Other();
    }
    return found.answer() == null ? new Begin.Running() : new Begin.Replay(found.answer());
  }

  @Override
  public synchronized void complete(String scope, String key, StoredAnswer answer) {
    List<String> id = List.of(scope, key);
    Entry found = entries.get(id);
    if (found != null) {
      entries.put(id, new Entry(found.fingerprint(), answer));
    }
  }

  @Override
  public synchronized void release(String scope, String key) {
    entries.remove(List.of(scope, key));
  }
}
