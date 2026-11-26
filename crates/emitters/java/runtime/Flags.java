package dev.veld.runtime;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Function;
import java.util.function.Supplier;

/**
 * Flags (spec/platform.md section 11): {@code when flag("name")} on a statement says it exists only
 * while the flag is on. The server asks the {@link Provider} once per request for every flag the
 * endpoint can reach, after authentication, and keeps the answers in a view of the request that the
 * generated checks read; a client has no provider and decides from the answers of a function the
 * caller gives, or as a failing provider does. {@code isOn} is synchronous.
 *
 * <p>Off means the statement is not there. When the provider fails, the statements that restrict (an
 * invariant, a {@code require}, an {@code ensure}, a {@code forbid}) count as on and the ones that
 * grant (a {@code permit}, a {@code flow}) as off; the failure goes to {@code onError}.
 *
 * <p>The view is a thread-local that every window of a request puts in force and restores, because
 * checks nest (a refusal builds an error whose own rules are checked).
 */
public final class Flags {
  private Flags() {}

  /** What a flag provider knows about the request. */
  public record Context(Object principal, String tenant, String operation) {}

  /** Tells whether a flag is on. A failure counts as described in {@link Flags}. */
  @FunctionalInterface
  public interface Provider {
    /**
     * Whether the flag is on for this request.
     *
     * @param name the flag
     * @param context the caller (the principal, {@code null} without one), the caller's tenant text
     *     ({@code null} without one) and the operation signature
     * @throws Exception when the provider cannot answer
     */
    boolean isOn(String name, Context context) throws Exception;
  }

  /** Where the server asks: the provider of the application, or the test handler's header. */
  @FunctionalInterface
  interface Source {
    boolean isOn(String name, RequestContext request, Context context) throws Exception;
  }

  /** A provider for tests: the flags it was given are on. */
  public static final class Memory implements Provider {
    private final Set<String> on = ConcurrentHashMap.newKeySet();

    /** Creates a provider in which the given flags are on. */
    public Memory(Collection<String> names) {
      on.addAll(names);
    }

    /** Turns a flag on or off. */
    public Memory set(String name, boolean value) {
      if (value) {
        on.add(name);
      } else {
        on.remove(name);
      }
      return this;
    }

    @Override
    public boolean isOn(String name, Context context) {
      return on.contains(name);
    }
  }

  /** What one request knows of one flag. */
  enum State {
    ON,
    OFF,
    FAILED
  }

  /** The readings of one request, or the answers of a caller's function. */
  static final class View {
    private final Map<String, State> readings = new LinkedHashMap<>();
    private final Function<String, Boolean> answers;

    View(Function<String, Boolean> answers) {
      this.answers = answers;
    }

    void put(String name, State state) {
      readings.put(name, state);
    }

    State of(String name) {
      if (answers != null) {
        return Boolean.TRUE.equals(answers.apply(name)) ? State.ON : State.OFF;
      }
      return readings.getOrDefault(name, State.FAILED);
    }
  }

  private static final ThreadLocal<View> CURRENT = new ThreadLocal<>();

  static View current() {
    return CURRENT.get();
  }

  /** Puts a view in force on this thread, and returns the one it replaces. */
  static View install(View view) {
    View before = CURRENT.get();
    set(view);
    return before;
  }

  /** Restores the view {@link #install} returned. */
  static void restore(View before) {
    set(before);
  }

  private static void set(View view) {
    if (view == null) {
      CURRENT.remove();
    } else {
      CURRENT.set(view);
    }
  }

  /** Runs {@code body} with the view of a request in force, and restores the previous one. */
  static <T> T with(View view, Supplier<T> body) {
    View before = install(view);
    try {
      return body.get();
    } finally {
      restore(before);
    }
  }

  /**
   * Reads every flag of {@code names} once, in order, through {@code source}. A flag whose reading
   * fails is {@code FAILED}; the failure is handed to {@code failed}.
   */
  static View read(
      List<String> names, Source source, RequestContext request, Context context, java.util.function.Consumer<Throwable> failed) {
    View view = new View(null);
    for (String name : names) {
      try {
        view.put(name, source.isOn(name, request, context) ? State.ON : State.OFF);
      } catch (Exception | StackOverflowError e) {
        view.put(name, State.FAILED);
        failed.accept(e);
      }
    }
    return view;
  }

  /**
   * Whether a statement that is not an invariant (a {@code require}, an {@code ensure}, a {@code
   * permit}, a {@code forbid}, a {@code flow}) is there: on when the flag is on, off when it is
   * off, and {@code whenFailed} when the provider failed or there is no provider to ask.
   */
  public static boolean on(String name, boolean whenFailed) {
    View view = CURRENT.get();
    if (view == null) {
      return whenFailed;
    }
    return switch (view.of(name)) {
      case ON -> true;
      case OFF -> false;
      case FAILED -> whenFailed;
    };
  }

  /**
   * Whether a flagged invariant is evaluated: with no view (a client's local check, {@code partial})
   * it is not, as an audited rule is not; a failed reading counts as on.
   */
  public static boolean invariantOn(String name) {
    View view = CURRENT.get();
    if (view == null) {
      return false;
    }
    return view.of(name) != State.OFF;
  }

  /** The states of {@code states} whose edge is there: an edge with no flag always, a flagged one while its flag is on ({@code flow} grants, so a failed reading is off). */
  public static <S> List<S> sources(S[] states, String[] flags) {
    List<S> out = new ArrayList<>();
    for (int i = 0; i < states.length; i++) {
      if (flags[i] == null || on(flags[i], false)) {
        out.add(states[i]);
      }
    }
    return out;
  }

  /**
   * Runs {@code body} as a server whose provider failed for every flag: what restricts counts as on
   * and what grants as off. For the shared vectors ({@code "provider": "failed"}).
   */
  public static <T> T withFailedProvider(Supplier<T> body) {
    return with(new View(null), body);
  }

  /**
   * Runs {@code body} with the answers of a function from a flag name to a boolean in force: what a
   * client's {@code can<Endpoint>} and policy functions do with their optional last argument.
   * Without the function ({@code null}) they decide as a failing provider does.
   */
  public static <T> T withAnswers(Function<String, Boolean> answers, Supplier<T> body) {
    return with(answers == null ? null : new View(answers), body);
  }
}
