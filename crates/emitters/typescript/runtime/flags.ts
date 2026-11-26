// Feature flags (`when flag("name")`, spec/platform.md section 11).
//
// A statement behind a flag exists only while the flag is on. The check functions of the models and
// the policy functions ask this module, and what they are told is the reading of the request that
// is running them. Rule checks run synchronously, so one module-level view is safe: it is set right
// before a walk and put back right after it, with no `await` in between, and walks nest (a refusal
// builds an error whose rules are checked), so the previous view is restored.

/** What one request knows about one flag: on, off, or the provider failed. */
export type FlagReading = 'on' | 'off' | 'failed';

type View = (name: string) => FlagReading;

let view: View | null = null;

/** Runs `body` with the readings of one request in force. */
export function withReadings<T>(readings: View | null, body: () => T): T {
  const saved = view;
  view = readings;
  try {
    return body();
  } finally {
    view = saved;
  }
}

/**
 * Runs `body` with the answers of a function a caller gave (`canBook(..., flags)`): a flag is on
 * when the function says so. Without one, the caller has no provider and a flag reads as a failing
 * provider reads (`flagOn`).
 */
export function withAnswers<T>(answers: ((name: string) => boolean) | undefined, body: () => T): T {
  return withReadings(answers === undefined ? null : (name) => (answers(name) ? 'on' : 'off'), body);
}

/**
 * Whether a statement that grants (a `permit`, a `flow`) or restricts (a `forbid`) is there.
 * `whenFailed` is what a failing provider makes of it, and what a reader with no provider gets.
 */
export function flagOn(name: string, whenFailed: boolean): boolean {
  if (view === null) return whenFailed;
  const reading = view(name);
  return reading === 'failed' ? whenFailed : reading === 'on';
}

/**
 * Whether a flagged invariant is evaluated. A failing provider counts as on. Without a reader (a
 * client's local check and `partial`) it is not evaluated: the server decides.
 */
export function invariantOn(name: string): boolean {
  return view !== null && flagOn(name, true);
}
