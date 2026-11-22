package dev.veld.runtime;

/** A non-2xx response that is not one of the endpoint's declared errors. */
public final class VeldHttpError extends RuntimeException {
  private static final long serialVersionUID = 1L;

  private final int status;
  private final transient Json body;

  /**
   * Creates the error.
   *
   * @param status the HTTP status
   * @param body the parsed response body, or {@code null} when it was empty
   */
  public VeldHttpError(int status, Json body) {
    super("HTTP " + status);
    this.status = status;
    this.body = body;
  }

  /** The HTTP status. */
  public int status() {
    return status;
  }

  /** The parsed response body, or {@code null} when it was empty. */
  public Json body() {
    return body;
  }
}
