package dev.veld.runtime;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * What the router does about the conditional headers of one endpoint (spec/rules.md sections 11.9
 * and 11.10): the {@code If-Match} guard, the {@code ETag} of a success and the idempotency key.
 * The generated {@code <Service>Handler} hands the router one of these per endpoint that needs it,
 * by operation signature.
 *
 * @param etag the wire name of the version field of the success model, or {@code null} when the
 *     success owes no {@code ETag}
 * @param guard whether the endpoint has an {@code If-Match} guard: the header is then read and
 *     checked at step 5
 * @param idempotency the idempotency key of the endpoint, or {@code null}
 */
public record Conditions(String etag, boolean guard, Idempotency idempotency) {

  /**
   * The {@code @idempotent} annotation of an endpoint.
   *
   * @param header the request header that carries the key
   * @param required whether a request without the header is {@code 422}
   * @param caller whether the scope of a key includes the caller
   * @param conflictSig the declaration signature of the {@code Conflict} error
   * @param conflictStatus the status the endpoint answers {@code Conflict} with
   */
  public record Idempotency(
      String header, boolean required, boolean caller, String conflictSig, int conflictStatus) {}

  /** The {@code message} of the {@code 409 Conflict} for a key held with another fingerprint. */
  static final String OTHER_MESSAGE = "this idempotency key was used with other parameters";

  /** The {@code message} of the {@code 409 Conflict} for a key whose first request is running. */
  static final String RUNNING_MESSAGE = "a request with this idempotency key is still running";

  private static final String IF_MATCH_MESSAGE =
      "must be * or one strong entity tag holding a version, such as \"3\"";
  private static final String KEY_MESSAGE = "must be 1 to 255 visible ASCII characters";

  /** The longest key, in characters. */
  private static final int MAX_KEY = 255;

  /** What a well-formed {@code If-Match} says: {@code *}, or the version of one strong tag. */
  private record Tag(boolean any, long version) {}

  /**
   * Step 5 of the pipeline for the two headers: the issues of a malformed {@code If-Match} (on an
   * endpoint with a guard) and of a malformed or missing key. The version of a well-formed tag is
   * left on the context for the guard of step 6. Neither check is part of validating the
   * parameters, so {@code validate = false} does not switch them off.
   */
  List<Issue> issues(RequestContext ctx) {
    List<Issue> out = new ArrayList<>();
    if (guard) {
      List<String> sent = ctx.headers().get("if-match");
      if (sent != null && !sent.isEmpty()) {
        Tag tag = sent.size() == 1 ? parse(sent.get(0)) : null;
        if (tag == null) {
          out.add(new Issue("header.If-Match", "type", IF_MATCH_MESSAGE));
        } else if (!tag.any()) {
          ctx.setIfMatchVersion(tag.version());
        }
      }
    }
    if (idempotency != null) {
      List<String> sent = ctx.headers().get(idempotency.header().toLowerCase(Locale.ROOT));
      String path = "header." + idempotency.header();
      if (sent == null || sent.isEmpty()) {
        if (idempotency.required()) {
          out.add(new Issue(path, "required", "is required"));
        }
      } else if (sent.size() != 1 || !validKey(sent.get(0))) {
        out.add(new Issue(path, "type", KEY_MESSAGE));
      }
    }
    return out;
  }

  /** The key of the request, or {@code null} when it carries none. */
  String key(RequestContext ctx) {
    if (idempotency == null) {
      return null;
    }
    List<String> sent = ctx.headers().get(idempotency.header().toLowerCase(Locale.ROOT));
    return sent == null || sent.isEmpty() ? null : sent.get(0);
  }

  /** {@code *}, or one strong entity tag whose text is a version; {@code null} for anything else. */
  private static Tag parse(String text) {
    if (text.equals("*")) {
      return new Tag(true, 0L);
    }
    if (text.length() < 3 || text.charAt(0) != '"' || text.charAt(text.length() - 1) != '"') {
      return null;
    }
    String digits = text.substring(1, text.length() - 1);
    boolean canonical = digits.equals("0") || (digits.charAt(0) >= '1' && digits.charAt(0) <= '9');
    for (int i = 0; canonical && i < digits.length(); i++) {
      canonical = digits.charAt(i) >= '0' && digits.charAt(i) <= '9';
    }
    if (!canonical) {
      return null;
    }
    try {
      return new Tag(false, Long.parseLong(digits));
    } catch (NumberFormatException e) {
      return null;
    }
  }

  /** Whether {@code key} is 1 to 255 visible ASCII characters. */
  static boolean validKey(String key) {
    if (key.isEmpty() || key.length() > MAX_KEY) {
      return false;
    }
    for (int i = 0; i < key.length(); i++) {
      char c = key.charAt(i);
      if (c < 0x21 || c > 0x7e) {
        return false;
      }
    }
    return true;
  }

  /**
   * The {@code ETag} of a success: the version of the returned value as a strong entity tag, or
   * {@code null} when the endpoint owes none or the value carries no version.
   */
  String etagOf(Json body) {
    if (etag == null || !(body instanceof Json.Obj obj)) {
      return null;
    }
    Json version = obj.get(etag);
    if (version instanceof Json.Str s) {
      return "\"" + s.value() + "\"";
    }
    if (version instanceof Json.Num n) {
      return "\"" + (long) n.value() + "\"";
    }
    return null;
  }
  // #if early

  /**
   * Thrown by a test handler to end a request with a response that is already built (a dry run).
   * A server that has endpoint rules uses {@code EndpointRules.Early} for this.
   */
  public static final class Early extends RuntimeException {
    private static final long serialVersionUID = 1L;
    private final transient VeldResponse response;

    /** Ends the request with {@code response}. */
    public Early(VeldResponse response) {
      super("the request ends with status " + response.status(), null, false, false);
      this.response = response;
    }

    /** The response. */
    public VeldResponse response() {
      return response;
    }
  }
  // #endif
}
