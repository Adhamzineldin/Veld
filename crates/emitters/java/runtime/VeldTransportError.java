package dev.veld.runtime;

/** A network failure, or a response body that cannot be understood. */
public final class VeldTransportError extends RuntimeException {
  private static final long serialVersionUID = 1L;

  /** Creates the error. */
  public VeldTransportError(String message) {
    super(message);
  }

  /** Creates the error with its cause. */
  public VeldTransportError(String message, Throwable cause) {
    super(message, cause);
  }
}
