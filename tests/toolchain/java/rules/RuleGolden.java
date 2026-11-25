package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.rg.adapters.JdkHttpServerAdapter;
import gen.rg.bookings.bookings.Booking;
import gen.rg.bookings.server.BookingsHandler;
import gen.rg.bookings.server.BookingsHooks;
import gen.rg.bookings.server.BookingsService;
import gen.rg.bookings.server.CreateRequest;
import gen.rg.bookings.server.ReportRequest;
import gen.rg.bookings.server.SearchRequest;
import gen.rg.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

/**
 * The 422 golden on the JDK server: sends the request of {@code tests/wire/rules/request.json}
 * (method, path and body come as arguments) and prints the status, the content type and the body.
 * The Rust test canonicalises the body and compares it with {@code problem.json}.
 */
public final class RuleGolden {
  private RuleGolden() {}

  public static void main(String[] args) throws Exception {
    BookingsService impl = new BookingsService() {
      @Override
      public Booking create(CreateRequest request, RequestContext ctx) {
        return request.body();
      }

      @Override
      public Integer search(SearchRequest request, RequestContext ctx) {
        return 0;
      }

      @Override
      public gen.rg.bookings.bookings.Report report(ReportRequest request, RequestContext ctx) {
        throw new UnsupportedOperationException();
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(
        BookingsHandler.create(impl, new BookingsHooks()), new InetSocketAddress("127.0.0.1", 0));
    try {
      HttpRequest request = HttpRequest.newBuilder(
              URI.create("http://127.0.0.1:" + server.getAddress().getPort() + args[1]))
          .header("content-type", "application/json")
          .method(args[0], HttpRequest.BodyPublishers.ofString(args[2]))
          .build();
      HttpResponse<String> response = HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString());
      System.out.println(response.statusCode());
      System.out.println(response.headers().firstValue("content-type").orElse(""));
      System.out.println(response.body());
    } finally {
      server.stop(0);
    }
  }
}
