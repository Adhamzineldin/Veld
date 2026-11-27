import { useMemo } from 'react';
import type { DeepPartial, Violation } from './veld';

/**
 * The rules of a draft, for a form: `useFormRules(Booking, draft)`. It re-checks when the draft is a
 * new object, so keep the draft in immutable state like any other React form state.
 */
export function useFormRules<T>(model: FormModel<T>, draft: DeepPartial<T>): FormRules {
  return useMemo(() => formRules(model.partial(draft)), [model, draft]);
}
