package veldtest;

import gen.sf.runtime.Json;
import gen.sf.runtime.SseEvent;
import gen.sf.runtime.SseParser;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * The golden file {@code tests/wire/streaming/sse.json} through the Java runtime: every frame
 * written by {@link SseEvent#frame} must equal the golden bytes, and every parse case must give
 * the golden events through {@link SseParser}, whatever way the chunks are split.
 */
public final class SseWire {
  private SseWire() {}

  public static void main(String[] args) throws Exception {
    Path file = Path.of(System.getenv("WIRE_DIR"), "streaming", "sse.json");
    Json.Obj golden = Json.parse(Files.readString(file, StandardCharsets.UTF_8)).asObject();
    int frames = 0;
    for (Json j : golden.get("frames").asArray().items()) {
      Json.Obj c = j.asObject();
      Json.Obj o = c.get("options").asObject();
      String id = o.get("id") == null ? null : o.get("id").asString();
      String event = o.get("event") == null ? null : o.get("event").asString();
      Integer retry = o.get("retry") == null ? null : (int) o.get("retry").asDouble();
      Check.equal(c.get("frame").asString(), SseEvent.frame(c.get("data").asString(), id, event, retry), c.get("name").asString());
      frames++;
    }
    int parsed = 0;
    for (Json j : golden.get("parse").asArray().items()) {
      Json.Obj c = j.asObject();
      List<String> chunks = new ArrayList<>();
      for (Json chunk : c.get("chunks").asArray().items()) {
        chunks.add(chunk.asString());
      }
      String name = c.get("name").asString();
      Check.equal(describe(c.get("events").asArray()), describe(run(chunks)), name);
      // The same text in one piece, and byte by byte, must give the same events.
      String all = String.join("", chunks);
      Check.equal(describe(run(chunks)), describe(run(List.of(all))), name + " (one piece)");
      List<String> single = new ArrayList<>();
      all.codePoints().forEach(cp -> single.add(new String(Character.toChars(cp))));
      Check.equal(describe(run(chunks)), describe(run(single)), name + " (one character at a time)");
      parsed++;
    }
    Check.that(frames >= 5 && parsed >= 10, "the golden file has its cases");
    System.out.println("sse wire ok (" + frames + " frames, " + parsed + " parse cases)");
  }

  private static List<SseParser.Message> run(List<String> chunks) {
    SseParser parser = new SseParser();
    List<SseParser.Message> out = new ArrayList<>();
    for (String chunk : chunks) {
      out.addAll(parser.push(chunk));
    }
    return out;
  }

  private static String describe(List<SseParser.Message> messages) {
    StringBuilder out = new StringBuilder();
    for (SseParser.Message m : messages) {
      out.append(m.event()).append('|').append(m.data()).append('|').append(m.id()).append('\n');
    }
    return out.toString();
  }

  private static String describe(Json.Arr events) {
    StringBuilder out = new StringBuilder();
    for (Json e : events.items()) {
      Json.Obj o = e.asObject();
      out.append(o.get("event").asString()).append('|').append(o.get("data").asString()).append('|')
          .append(o.get("id") == null ? null : o.get("id").asString()).append('\n');
    }
    return out.toString();
  }
}
