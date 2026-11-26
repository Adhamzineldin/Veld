package veldtest;

import gen.rs.live.live.Overbooked;
import gen.rs.live.live.Note;
import gen.rs.live.live.Reading;
import gen.rs.live.server.LiveHooks;
import gen.rs.live.server.LiveService;
import gen.rs.live.server.NotesRequest;
import gen.rs.live.server.ReadingsRequest;
import gen.rs.live.server.SeatsRequest;
import gen.rs.runtime.RequestContext;
import gen.rs.runtime.SseEvent;
import gen.rs.runtime.WsSession;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.stream.IntStream;
import java.util.stream.Stream;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Bean;

/**
 * The streams contract with rules on Spring: an SSE stream and a WebSocket whose server and client
 * messages have invariants. {@code --veld.rules.output=audit} or {@code off} sets the output mode; the
 * violations the hook saw are listed at {@code /__events}.
 */
@SpringBootApplication(scanBasePackages = {"veldtest", "gen.rs.adapters"})
public class App {
  static final List<String> EVENTS = new CopyOnWriteArrayList<>();

  public static void main(String[] args) {
    SpringApplication.run(App.class, args);
  }

  @Bean
  LiveService liveService() {
    return new LiveService() {
      @Override
      public Stream<SseEvent<Reading>> readings(ReadingsRequest request, RequestContext ctx) {
        int bad = request.bad();
        return IntStream.range(0, 3)
            .mapToObj(i -> SseEvent.of(i == bad ? new Reading(5, 1) : new Reading(1, 2)));
      }

      @Override
      public void notes(NotesRequest request, RequestContext ctx, WsSession<Note, Reading> ws) {
        ws.incoming().forEach(n -> ws.send(n.text().equals("bad") ? new Reading(9, 1) : new Reading(0, n.stars())));
      }

      @Override
      public Integer seats(SeatsRequest request, RequestContext ctx) {
        throw new Overbooked(new Overbooked.Data(2, 5));
      }
    };
  }

  @Bean
  LiveHooks liveHooks() {
    return new LiveHooks()
        .onRuleViolation(e -> EVENTS.add(e.position() + " " + e.outcome()));
  }
}
