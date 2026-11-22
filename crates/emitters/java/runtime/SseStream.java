package dev.veld.runtime;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.InterruptedIOException;
import java.io.Reader;
import java.net.http.HttpResponse;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.Collections;
import java.util.Deque;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.Flow;
import java.util.concurrent.LinkedBlockingQueue;
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

  /**
   * The body of an event stream as an {@link InputStream} that hands over every byte received
   * before the connection broke, and only then the failure. The JDK's {@code ofInputStream()}
   * throws as soon as the connection fails, dropping events that had arrived but were not read yet.
   */
  static HttpResponse.BodyHandler<InputStream> bodyHandler() {
    return info -> new Body();
  }

  /** Received buffers, the end, or the failure, in arrival order; the stream is ready at once. */
  private static final class Body implements HttpResponse.BodySubscriber<InputStream> {
    private record Chunk(List<ByteBuffer> buffers) {}

    private record Failure(Throwable cause) {}

    private static final Chunk END = new Chunk(List.of());

    private final BlockingQueue<Object> arrived = new LinkedBlockingQueue<>();
    private volatile Flow.Subscription subscription;
    private final Reading stream = new Reading();

    @Override
    public CompletionStage<InputStream> getBody() {
      return CompletableFuture.completedStage(stream);
    }

    @Override
    public void onSubscribe(Flow.Subscription s) {
      subscription = s;
      if (stream.over) {
        s.cancel();
      } else {
        s.request(1);
      }
    }

    @Override
    public void onNext(List<ByteBuffer> buffers) {
      arrived.add(new Chunk(buffers));
    }

    @Override
    public void onError(Throwable cause) {
      arrived.add(new Failure(cause));
    }

    @Override
    public void onComplete() {
      arrived.add(END);
    }

    private final class Reading extends InputStream {
      private ByteBuffer current = ByteBuffer.allocate(0);
      private Iterator<ByteBuffer> rest = Collections.emptyIterator();
      private IOException failure;
      private volatile boolean over;

      @Override
      public int read() throws IOException {
        byte[] one = new byte[1];
        return read(one, 0, 1) < 0 ? -1 : one[0] & 0xff;
      }

      @Override
      public int read(byte[] into, int off, int len) throws IOException {
        if (len == 0) {
          return 0;
        }
        while (!current.hasRemaining()) {
          if (rest.hasNext()) {
            current = rest.next();
          } else if (!advance()) {
            return -1;
          }
        }
        int n = Math.min(len, current.remaining());
        current.get(into, off, n);
        return n;
      }

      /** Takes the next arrival; {@code false} at the end of the body. */
      private boolean advance() throws IOException {
        if (failure != null) {
          throw failure;
        }
        if (over) {
          return false;
        }
        Object next;
        try {
          next = arrived.take();
        } catch (InterruptedException e) {
          Thread.currentThread().interrupt();
          throw new InterruptedIOException("interrupted while reading the event stream");
        }
        if (next instanceof Failure f) {
          failure = f.cause() instanceof IOException io ? io : new IOException(f.cause());
          throw failure;
        }
        if (next == END) {
          over = true;
          return false;
        }
        rest = ((Chunk) next).buffers().iterator();
        Flow.Subscription s = subscription;
        if (s != null) {
          s.request(1);
        }
        return true;
      }

      @Override
      public void close() {
        if (over) {
          return;
        }
        over = true;
        arrived.add(END);
        Flow.Subscription s = subscription;
        if (s != null) {
          s.cancel();
        }
      }
    }
  }
}
