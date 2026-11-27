package dev.veld.runtime;

/**
 * What {@code onError} hears when the audit log of a request could not be written (spec/platform.md
 * section 4): the {@code audit} hook failed, or the event could not be built (a value that does not
 * have the shape of its type cannot be known to be clean). The response of the request is sent
 * unchanged.
 */
public final class AuditFailure extends RuntimeException {
  private static final long serialVersionUID = 1L;
  private final transient AuditEvent event;

  AuditFailure(String reason, AuditEvent event, Throwable cause) {
    super(reason, cause);
    this.event = event;
  }

  /** Why the event was not recorded. */
  public String reason() {
    return getMessage();
  }

  /** The event the hook was given, or {@code null} when it could not be built. */
  public AuditEvent event() {
    return event;
  }
}
