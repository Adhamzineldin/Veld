package dev.veld.runtime;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Iterator;
import java.util.NoSuchElementException;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/**
 * The client side of one Server-Sent Events response: reads the body as it arrives, parses it with
 * {@link SseParser} and yields the wire value of every event. A final {@code error} event throws
 * the typed error, a broken connection throws {@link VeldTransportError}. Closing the stream
 * cancels the request.
 */
final class SseStream implements Iterator<Json> {
  private final EndpointSpec ep;
  /** Opens the response body again, resuming after {@code lastEventId} (may be {@code null}). */
  @FunctionalInterface
  interface Reopen {
    InputStream open(String lastEventId);
  }

  private InputStream body;
  private Reader reader;
  private final SseOptions options;
  private final Reconnect reconnect;
  private final Reopen reopen;
  private SseParser parser = new SseParser();
  private String lastId;
  private boolean delivered;
  private int failures;
  private final Deque<SseParser.Message> queue = new ArrayDeque<>();
  private final char[] buffer = new char[4096];
  private volatile boolean closed;
  private boolean done;

  private SseStream(
      EndpointSpec ep, InputStream body, SseOptions options, Reconnect reconnect, Reopen reopen) {
    this.ep = ep;
    this.body = body;
    this.reader = new InputStreamReader(body, StandardCharsets.UTF_8);
    this.options = options;
    this.reconnect = reconnect;
    this.reopen = reopen;
    this.lastId = options.lastEventId();
  }

  /** The events of {@code body} as a stream of wire values; closing it closes {@code body}. */
  static Stream<Json> open(
      EndpointSpec ep, InputStream body, SseOptions options, Reconnect reconnect, Reopen reopen) {
    SseStream events = new SseStream(ep, body, options, reconnect, reopen);
    return StreamSupport.stream(
            Spliterators.spliteratorUnknownSize(events, Spliterator.ORDERED | Spliterator.NONNULL),
            false)
        .onClose(events::close);
  }

  @Override
  public boolean hasNext() {
    while (queue.isEmpty() && !done) {
      fill();
    }
    return !queue.isEmpty();
  }

  @Override
  public Json next() {
    if (!hasNext()) {
      throw new NoSuchElementException();
    }
    SseParser.Message message = queue.poll();
    if (message.id() != null) {
      lastId = message.id();
      if (options.onEventId() != null) {
        options.onEventId().accept(message.id());
      }
    }
    delivered = true;
    return value(message);
  }

  private void fill() {
    int n;
    try {
      n = reader.read(buffer);
    } catch (IOException e) {
      if (closed) {
        n = -1;
      } else if (reconnect != null && reopen != null) {
        resume(e);
        return;
      } else {
        close();
        throw new VeldTransportError(ep.method() + " " + ep.path() + ": the stream broke", e);
      }
    }
    if (n < 0) {
      close();
      return;
    }
    queue.addAll(parser.push(new String(buffer, 0, n)));
  }

  /** The connection broke: wait, then open the request again with {@code Last-Event-ID}. */
  private void resume(IOException failure) {
    IOException cause = failure;
    if (delivered) {
      failures = 0;
      delivered = false;
    }
    try {
      body.close();
    } catch (IOException e) {
      // The connection is already gone.
    }
    while (true) {
      if (closed) {
        return;
      }
      if (failures >= reconnect.retries()) {
        close();
        throw new VeldTransportError(ep.method() + " " + ep.path() + ": the stream broke", cause);
      }
      Reconnect.sleep(reconnect.backoffMs(failures));
      failures++;
      try {
        InputStream next = reopen.open(lastId);
        body = next;
        reader = new InputStreamReader(next, StandardCharsets.UTF_8);
        parser = new SseParser();
        return;
      } catch (VeldTransportError e) {
        cause = new IOException(e.getMessage(), e);
      }
    }
  }

  private Json value(SseParser.Message message) {
    Json parsed;
    try {
      parsed = Json.parse(message.data());
    } catch (Json.ParseException e) {
      abort();
      throw new VeldTransportError(ep.method() + " " + ep.path() + ": an event is not valid JSON", e);
    }
    if ("error".equals(message.event())) {
      abort();
      int status = parsed instanceof Json.Obj obj && obj.get("status") instanceof Json.Num n ? (int) n.value() : 0;
      throw ClientCore.decodeFailure(ep, status, parsed);
    }
    if (ep.success() == null) {
      abort();
      throw new VeldTransportError(ep.method() + " " + ep.path() + ": the stream has no item type");
    }
    return Wire.prepare(ep.success(), parsed, Wire.Mode.RESPONSE);
  }

  /** Ends the stream after a failure: the events behind it are dropped. */
  private void abort() {
    queue.clear();
    close();
  }

  void close() {
    closed = true;
    done = true;
    try {
      body.close();
    } catch (IOException e) {
      // Closing is best effort.
    }
  }
}
