package dev.veld.runtime;

import java.util.List;
// veld:sse
import java.util.stream.Stream;
// veld:end

/**
 * An endpoint of a server: its description, its middleware and the call into the implementation.
 *
 * @param spec the endpoint description
 * @param middleware the middleware names, in declared order
 * @param call decodes the bound parameters, calls the implementation and encodes the result;
 *     {@code null} for a streaming endpoint
 * @param sse the call of a Server-Sent Events endpoint, or {@code null}
 * @param ws the call of a {@code ws} endpoint, or {@code null}
 */
public record ServerEndpoint(
    EndpointSpec spec,
    List<String> middleware,
    Invoker call
    // veld:sse
    , SseInvoker sse
    // veld:end
    // veld:ws
    , WsInvoker ws
    // veld:end
    ) {

  /** Creates an endpoint (the middleware list is copied). */
  public ServerEndpoint {
    middleware = List.copyOf(middleware);
  }

  // veld:streaming
  /** Creates an ordinary endpoint. */
  public ServerEndpoint(EndpointSpec spec, List<String> middleware, Invoker call) {
    this(spec, middleware, call
        // veld:sse
        , null
        // veld:end
        // veld:ws
        , null
        // veld:end
        );
  }

  // veld:end
  // veld:sse
  /** Creates a Server-Sent Events endpoint. */
  public static ServerEndpoint sse(EndpointSpec spec, List<String> middleware, SseInvoker call) {
    return new ServerEndpoint(spec, middleware, null, call
        // veld:ws
        , null
        // veld:end
        );
  }

  // veld:end
  // veld:ws
  /** Creates a {@code ws} endpoint. */
  public static ServerEndpoint ws(EndpointSpec spec, List<String> middleware, WsInvoker call) {
    return new ServerEndpoint(spec, middleware, null
        // veld:sse
        , null
        // veld:end
        , call);
  }

  // veld:end
  /** Calls the implementation. */
  @FunctionalInterface
  public interface Invoker {
    /**
     * Calls the implementation.
     *
     * @param params the bound and validated parameters, by parameter name, in wire form
     * @param ctx the request context
     * @return the result in wire form, or {@code null} for an endpoint without a response body
     */
    Json call(Json.Obj params, RequestContext ctx) throws Exception;
  }

  // veld:sse
  /** Calls the implementation of a Server-Sent Events endpoint. */
  @FunctionalInterface
  public interface SseInvoker {
    /**
     * Calls the implementation.
     *
     * @return the events in wire form; the router closes the stream when the client goes away
     */
    Stream<SseEvent<Json>> call(Json.Obj params, RequestContext ctx) throws Exception;
  }

  // veld:end
  // veld:ws
  /** Calls the implementation of a {@code ws} endpoint. */
  @FunctionalInterface
  public interface WsInvoker {
    /** Calls the implementation; returning closes the socket normally. */
    void call(Json.Obj params, RequestContext ctx, WsSession<Json, Json> session) throws Exception;
  }
  // veld:end
}
