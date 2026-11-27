// The React form helper of a contract with rules (spec/targets/typescript.md 4a): `useFormRules(Booking, draft)`
// over `Booking.partial`. Expects `./client_react` (typescript, framework react) generated from
// tests/toolchain/contract/rules.
import assert from 'node:assert/strict';
import { createElement } from 'react';
import { act, create } from 'react-test-renderer';
import * as cli from './client_react/index';
import { formRules, useFormRules } from './client_react/bookings/react';
import type { FormRules } from './client_react/bookings/react';
import type { DeepPartial } from './client_react/runtime/veld';

const END_BEFORE_START = { start: '2026-01-02T00:00:00Z', end: '2026-01-01T00:00:00Z', guests: 11 };

function Form(props: { draft: DeepPartial<cli.Booking>; seen: FormRules[] }): null {
  props.seen.push(useFormRules(cli.Booking, props.draft));
  return null;
}

function last(seen: FormRules[]): FormRules {
  const r = seen[seen.length - 1];
  assert.ok(r !== undefined);
  return r;
}

function main(): void {
  const seen: FormRules[] = [];
  const bad = END_BEFORE_START;
  const tree = create(createElement(Form, { draft: bad, seen }));
  // Messages by the fields their rules read; the party rule reads `guests`, the order rule `end` and `start`.
  assert.equal(last(seen).valid, false);
  assert.equal(last(seen).violations.length, 2);
  assert.deepEqual(last(seen).messages, {
    end: ['A booking must end after it starts'],
    start: ['A booking must end after it starts'],
    guests: ['violates rule party'],
  });

  // The same draft object: the result is the memoised one, not a recomputation.
  const before = last(seen);
  act(() => tree.update(createElement(Form, { draft: bad, seen })));
  assert.equal(last(seen), before);

  // An incomplete draft is not invalid: a rule that needs an absent field is skipped.
  act(() => tree.update(createElement(Form, { draft: { guests: 2 }, seen })));
  assert.deepEqual(last(seen).messages, {});
  assert.equal(last(seen).valid, true);

  // A draft fixed field by field becomes valid.
  act(() => tree.update(createElement(Form, { draft: { ...bad, end: '2026-01-03T00:00:00Z', guests: 3 }, seen })));
  assert.equal(last(seen).valid, true);
  assert.deepEqual(last(seen).violations, []);
  tree.unmount();

  // The pure function groups by the fields a violation names, below the path of the value that carries the rule.
  const grouped = formRules([
    { rule: 'r1', path: '$', fields: ['a'], message: 'm1' },
    { rule: 'r2', path: '$.rows[1]', fields: ['qty', 'price'], message: 'm2' },
    { rule: 'r3', path: '$.rows[1]', fields: ['qty'], message: 'm3' },
  ]);
  assert.deepEqual(grouped.messages, { a: ['m1'], 'rows[1].qty': ['m2', 'm3'], 'rows[1].price': ['m2'] });
  assert.equal(formRules([]).valid, true);
  console.log('rules form react ok');
}

main();
