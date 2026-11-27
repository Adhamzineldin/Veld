
// -- rule forms (appended only when a contract has rules) -------------------------------------

/** What a form needs from a model's companion: the partial check of a draft (`Booking.partial`). */
export interface FormModel<T> {
  partial(draft: DeepPartial<T>): Violation[];
  /** Only there so that `T` is inferred from the companion. */
  of(value: T): T;
}

/** The state of the rules for one draft. */
export interface FormRules {
  /** Every violation of the draft's invariants. A rule that needs a field the draft lacks is skipped, not failed. */
  violations: Violation[];
  /**
   * Field name to the messages of the rules that read it, in the order of `violations`. Names are
   * wire names; a field of a nested value is `address.city` or `rows[1].qty`.
   */
  messages: Record<string, string[]>;
  /** `true` when the draft currently has no violations. */
  valid: boolean;
}

function fieldKey(path: string, field: string): string {
  const prefix = path.replace(/^\$\.?/, '');
  return prefix === '' ? field : `${prefix}.${field}`;
}

/** Groups the messages of `violations` by the fields their rules read (each violation's `fields`). */
export function formRules(violations: Violation[]): FormRules {
  const messages: Record<string, string[]> = {};
  for (const v of violations) {
    for (const field of v.fields) {
      const key = fieldKey(v.path, field);
      const list = messages[key] ?? [];
      list.push(v.message);
      messages[key] = list;
    }
  }
  return { violations, messages, valid: violations.length === 0 };
}
