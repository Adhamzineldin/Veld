package dev.veld.runtime;

import java.time.Instant;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * The deadline of one request (spec/platform.md section 12): the instant the request arrived plus
 * the endpoint's {@code @timeout}. The router runs the pipeline of a request that has one on a
 * worker thread and waits for it; when the deadline passes first it answers {@code 504
 * DeadlineExceeded}, drops whatever the pipeline answers later, and interrupts the worker once. The
 * handler cannot be stopped, so one that writes must notice the deadline itself ({@link
 * RequestContext#expired()}) or the interrupt.
 *
 * <p>The idempotency key of a request that hit its deadline is settled at the deadline (a {@code 5xx}
 * is not stored, so the key is released). What the pipeline settles later must be a no-op, or it
 * would release the key of a retry that took it in between: {@link #claim()} lets exactly one
 * settlement through.
 */
final class Deadline {
  private static final ThreadLocal<Deadline> CURRENT = new ThreadLocal<>();

  private final Instant at;
  private final long ms;
  private volatile boolean fired;
  private final AtomicBoolean settled = new AtomicBoolean();
  private volatile Runnable release;

  Deadline(long ms) {
    this.ms = ms;
    this.at = Instant.now().plusMillis(ms);
  }

  /** The deadline of the request that runs on this thread, or {@code null}. */
  static Deadline current() {
    return CURRENT.get();
  }

  /** Makes this the deadline of the request that runs on this thread. */
  void enter() {
    CURRENT.set(this);
  }

  /** Forgets the deadline of this thread. */
  static void leave() {
    CURRENT.remove();
  }

  Instant at() {
    return at;
  }

  long ms() {
    return ms;
  }

  /** Whether the router has already answered {@code 504} for this request. */
  boolean fired() {
    return fired;
  }

  /** Whether the instant has passed, or the router has answered. */
  boolean expired() {
    return fired || !Instant.now().isBefore(at);
  }

  /**
   * Registers the release of the idempotency key this request holds. A deadline that has fired
   * already releases it at once.
   */
  void lease(Runnable releaseKey) {
    this.release = releaseKey;
    if (fired) {
      settleAtDeadline();
    }
  }

  /** Lets one settlement of the key through: the pipeline's own, or the deadline's. */
  boolean claim() {
    return settled.compareAndSet(false, true);
  }

  /** The deadline has passed and the router answers {@code 504}: settles the key and marks the request. */
  void fire() {
    fired = true;
    settleAtDeadline();
  }

  private void settleAtDeadline() {
    Runnable r = release;
    if (r != null && claim()) {
      r.run();
    }
  }
}
