package veldtest;

import com.sun.net.httpserver.HttpServer;
import gen.nvs.adapters.JdkHttpServerAdapter;
import gen.nvc.greeter.client.GreeterClient;
import gen.nvc.greeter.client.HelloRequest;
import gen.nvs.greeter.greeter.Greeting;
import gen.nvs.greeter.server.GreeterHandler;
import gen.nvs.greeter.server.GreeterService;
import gen.nvc.runtime.ClientOptions;
import gen.nvs.runtime.RequestContext;
import java.net.InetSocketAddress;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

/**
 * {@code validate = false}: the Java server and client generated with the option skip run-time
 * request validation (the record types are unchanged), and {@code validate(true)} brings the
 * client check back.
 */
public final class NoValidate {
  private NoValidate() {}

  public static void main(String[] args) throws Exception {
    GreeterService impl = new GreeterService() {
      @Override
      public Greeting hello(gen.nvs.greeter.server.HelloRequest request, RequestContext ctx) {
        return new Greeting("hello " + request.name());
      }
    };
    HttpServer server = JdkHttpServerAdapter.listen(GreeterHandler.create(impl), new InetSocketAddress("127.0.0.1", 0));
    String base = "http://127.0.0.1:" + server.getAddress().getPort();

    // The server does not validate: a name shorter than 3 reaches the implementation.
    HttpResponse<String> raw = HttpClient.newHttpClient().send(
        HttpRequest.newBuilder(java.net.URI.create(base + "/greeter/hello?name=ab")).build(),
        HttpResponse.BodyHandlers.ofString());
    Check.equal(200, raw.statusCode(), "the server did not validate the request");

    // The client does not validate by default: the request is sent.
    GreeterClient client = new GreeterClient(ClientOptions.builder().baseUrl(base).build());
    Check.equal("hello ab", client.hello(new HelloRequest("ab")).text(), "unvalidated call goes through");

    // Asking for validation restores the local check.
    GreeterClient strict = new GreeterClient(ClientOptions.builder().baseUrl(base).validate(true).build());
    Check.thrown(gen.nvc.prelude.ValidationError.class, "validate(true) rejects locally",
        () -> strict.hello(new HelloRequest("ab")));
    server.stop(0);
    System.out.println("no-validate ok");
  }
}
