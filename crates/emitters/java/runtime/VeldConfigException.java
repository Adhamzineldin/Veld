package dev.veld.runtime;

import java.util.List;

/**
 * A runtime config that is not valid. It carries every problem at once; its message is {@code
 * invalid runtime config: } followed by the messages of the problems in path order, separated by
 * {@code "; "}.
 */
public final class VeldConfigException extends RuntimeException {
  private static final long serialVersionUID = 1L;

  /**
   * One problem of a runtime config.
   *
   * @param path the path of the setting, such as {@code services.iam.url}
   * @param message the message, which starts with the path
   */
  public record Problem(String path, String message) {}

  private final transient List<Problem> problems;

  /**
   * Creates the exception.
   *
   * @param message the text of the whole error
   * @param problems the problems, in path order
   */
  public VeldConfigException(String message, List<Problem> problems) {
    super(message);
    this.problems = List.copyOf(problems);
  }

  /** Every problem, sorted by path and then by message. */
  public List<Problem> problems() {
    return problems;
  }
}
