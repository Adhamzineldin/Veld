package dev.veld.runtime;

/** An error whose declaration is not part of the generated code, used for local fallbacks. */
public final class UntypedError extends VeldError {
  private static final long serialVersionUID = 1L;

  private final transient Json data;

  /** Creates an untyped error with its data in wire form. */
  public UntypedError(String sig, String title, Json data, int status, String op) {
    super(sig, title, status, op);
    this.data = data;
  }

  @Override
  public Json data() {
    return data;
  }

  @Override
  public Json dataWire() {
    return data;
  }

  @Override
  public Schema dataSchema() {
    return Schema.json();
  }
}
