// The Vue form helper of a contract with rules (spec/targets/typescript.md 4a): `useFormRules(Booking, draft)`
// over `Booking.partial`. Expects `./client_vue` (typescript, framework vue) generated from
// tests/toolchain/contract/rules.
import assert from 'node:assert/strict';
import { effectScope, reactive, ref } from 'vue';
import * as cli from './client_vue/index';
import { formRules, useFormRules } from './client_vue/bookings/vue';
import type { DeepPartial } from './client_vue/runtime/veld';

function main(): void {
  const scope = effectScope();
  scope.run(() => {
    // A ref draft: the computed values follow it.
    const draft = ref<DeepPartial<cli.Booking>>({ start: '2026-01-02T00:00:00Z', end: '2026-01-01T00:00:00Z', guests: 11 });
    const form = useFormRules(cli.Booking, draft);
    assert.equal(form.valid.value, false);
    assert.equal(form.violations.value.length, 2);
    assert.deepEqual(form.messages.value, {
      end: ['A booking must end after it starts'],
      start: ['A booking must end after it starts'],
      guests: ['violates rule party'],
    });
    draft.value = { guests: 2 };
    assert.equal(form.valid.value, true);
    assert.deepEqual(form.messages.value, {});

    // A getter, and a reactive object edited in place.
    const state = reactive<{ draft: DeepPartial<cli.Booking> }>({ draft: { guests: 2 } });
    const fromGetter = useFormRules(cli.Booking, () => state.draft);
    assert.equal(fromGetter.valid.value, true);
    state.draft.guests = 12;
    assert.deepEqual(fromGetter.messages.value, { guests: ['violates rule party'] });
    state.draft.guests = 3;
    assert.equal(fromGetter.valid.value, true);
  });
  scope.stop();

  const grouped = formRules([{ rule: 'r', path: '$.guest', fields: ['name'], message: 'm' }]);
  assert.deepEqual(grouped.messages, { 'guest.name': ['m'] });
  console.log('rules form vue ok');
}

main();
