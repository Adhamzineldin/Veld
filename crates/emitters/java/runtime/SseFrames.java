package dev.veld.runtime;

import java.util.Iterator;
import java.util.NoSuchElementException;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.function.Function;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/**
 * The frames of one Server-Sent Events response, pulled lazily from the implementation's stream.
 * The first event is pulled eagerly by {@link #open}: a failure before it is a normal error
 * response. A failure after it becomes the final {@code error} frame and ends the stream.
 */
final class SseFrames implements Iterator<String> {
  private final Stream<SseEvent<Json>> source;
  private final Iterator<SseEvent<Json>> items;
  private final Function<SseEvent<Json>, String> encode;
  private final Function<Throwable, String> onFailure;
  private String pending;
  private boolean done;

  private SseFrames(
      Stream<SseEvent<Json>> source,
      Iterator<SseEvent<Json>> items,
      Function<SseEvent<Json>, String> encode,
      Function<Throwable, String> onFailure,
      String first) {
    this.source = source;
    this.items = items;
    this.encode = encode;
    this.onFailure = onFailure;
    this.pending = first;
  }

  /**
   * Pulls the first event and returns the frames. Closing the returned stream closes {@code
   * source}.
   *
   * @param encode turns an event into its frame (and validates it)
   * @param onFailure turns a failure after the first event into the final error frame
   * @throws RuntimeException the failure of the source or of {@code encode} before the first event
   */
  static Stream<String> open(
      Stream<SseEvent<Json>> source,
      Function<SseEvent<Json>, String> encode,
      Function<Throwable, String> onFailure) {
    Iterator<SseEvent<Json>> items = source.iterator();
    String first = null;
    try {
      if (items.hasNext()) {
        first = encode.apply(items.next());
      }
    } catch (RuntimeException | StackOverflowError e) {
      closeQuietly(source);
      throw e;
    }
    SseFrames frames = new SseFrames(source, items, encode, onFailure, first);
    return StreamSupport.stream(
            Spliterators.spliteratorUnknownSize(frames, Spliterator.ORDERED | Spliterator.NONNULL),
            false)
        .onClose(frames::close);
  }

  @Override
  public boolean hasNext() {
    if (pending != null) {
      return true;
    }
    if (done) {
      return false;
    }
    try {
      if (items.hasNext()) {
        pending = encode.apply(items.next());
        return true;
      }
    } catch (RuntimeException | StackOverflowError e) {
      // The status line is long gone: the failure travels as a final `error` event.
      pending = onFailure.apply(e);
      close();
      return true;
    }
    close();
    return false;
  }

  @Override
  public String next() {
    if (!hasNext()) {
      throw new NoSuchElementException();
    }
    String frame = pending;
    pending = null;
    return frame;
  }

  void close() {
    done = true;
    closeQuietly(source);
  }

  private static void closeQuietly(Stream<?> stream) {
    try {
      stream.close();
    } catch (RuntimeException e) {
      // Closing is best effort.
    }
  }
}
