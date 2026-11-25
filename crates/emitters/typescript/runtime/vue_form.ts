import { computed } from 'vue';
import type { ComputedRef } from 'vue';
import type { DeepPartial, Violation } from './veld';

/** The rules of a draft as computed refs. */
export interface FormRulesRefs {
  violations: ComputedRef<Violation[]>;
  messages: ComputedRef<Record<string, string[]>>;
  valid: ComputedRef<boolean>;
}

/**
 * The rules of a draft, for a form: `useFormRules(Booking, draft)`. The draft may be a ref, a getter
 * or a reactive object; the result recomputes when it changes.
 */
export function useFormRules<T>(model: FormModel<T>, draft: MaybeRefOrGetter<DeepPartial<T>>): FormRulesRefs {
  const state = computed(() => formRules(model.partial(toValue(draft))));
  return {
    violations: computed(() => state.value.violations),
    messages: computed(() => state.value.messages),
    valid: computed(() => state.value.valid),
  };
}
