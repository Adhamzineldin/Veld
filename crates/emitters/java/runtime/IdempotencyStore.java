package dev.veld.runtime;

/**
 * Where an {@code @idempotent} endpoint keeps what it has answered (spec/rules.md section 11.9).
 * Implement it over a database or a cache; {@link MemoryIdempotencyStore} is for tests.
 *
 * <p>An entry is addressed by a {@code scope} and a {@code key}. The scope is one opaque text: the
 * operation signature and, for an endpoint whose scope includes the caller, a line feed and the
 * caller text. How long an entry is kept is the store's business.
 */
public interface IdempotencyStore {

  /**
   * Asks for the key. It is atomic: of two concurrent calls for a key the store does not know,
   * exactly one gets {@link Begin.Started}. A key held with another fingerprint is
   * {@link Begin.Other}, whether its request is running or its answer is stored.
   *
   * @param scope the scope of the key
   * @param key the key the caller sent
   * @param fingerprint the fingerprint of the request's parameters
   */
  Begin begin(String scope, String key, String fingerprint) throws Exception;

  /** Stores the answer of a request that {@link #begin} started. */
  void complete(String scope, String key, StoredAnswer answer) throws Exception;

  /** Removes the entry of a request that {@link #begin} started and that has no answer to keep. */
  void release(String scope, String key) throws Exception;

  /** What the store says about a key. */
  sealed interface Begin permits Begin.Started, Begin.Replay, Begin.Running, Begin.Other {

    /** Nothing was stored: the request runs, and the store now holds the key as running. */
    record Started() implements Begin {}

    /** The same key and fingerprint have an answer: it is sent again. */
    record Replay(StoredAnswer answer) implements Begin {}

    /** The key is held with the same fingerprint by a request that has not finished. */
    record Running() implements Begin {}

    /** The key is held with another fingerprint. */
    record Other() implements Begin {}
  }
}
