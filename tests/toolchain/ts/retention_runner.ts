// Retention (spec/platform.md section 23) in a generated server: `retentionPlan`, and
// `runApiRetention` with jobs that record what they are asked. Expects `./server` generated from
// tests/toolchain/contract/platform_p4/retention.veld (compiled as the package `bookings`, target
// `api`). Writes `plan.json` for the Rust test, which compares it with `Ir::retention_plan`.
// Prints `retention ok`.
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import * as srv from './server/index';

const DAY = 86_400_000;
const NOW = new Date('2026-06-10T12:00:00.000Z');

interface Call {
  job: 'delete' | 'anonymize';
  model: string;
  before: string;
  fields?: readonly string[];
}

function recording(overrides: Partial<srv.RetentionJobs> = {}): { jobs: srv.RetentionJobs; calls: Call[] } {
  const calls: Call[] = [];
  const jobs: srv.RetentionJobs = {
    delete: (model, before) => {
      calls.push({ job: 'delete', model, before: before.toISOString() });
      return 3;
    },
    anonymize: async (model, before, fields) => {
      calls.push({ job: 'anonymize', model, before: before.toISOString(), fields });
      return 2;
    },
    ...overrides,
  };
  return { jobs, calls };
}

const at = (days: number): string => new Date(NOW.getTime() - days * DAY).toISOString();

async function main(): Promise<void> {
  // The plan: one entry per model with `@retain`, sorted by name, with what each needs.
  writeFileSync('plan.json', JSON.stringify(srv.retentionPlan));
  const names = srv.retentionPlan.map((e) => e.model);
  assert.deepEqual(names, [...names].sort(), 'sorted by the qualified name');
  assert.deepEqual(
    srv.retentionPlan.map((e) => [e.model.split('.').pop(), e.afterMs / DAY, e.since, e.then, [...e.fields]]),
    [
      ['Guest', 30, 'checkedOutAt', 'anonymize', ['name', 'phone']],
      ['Invoice', 365, 'closedOn', 'delete', []],
      ['Staff', 90, 'leftAt', 'anonymize', ['name', 'badge']],
    ],
  );
  assert.ok(!names.some((n) => n.endsWith('.Kept')), 'a model without @retain is not in the plan');

  // One call per entry, in order, with `before` = now - afterMs, and the fields of an `anonymize`.
  let { jobs, calls } = recording();
  let report = await srv.runApiRetention(jobs, { now: NOW });
  assert.deepEqual(
    calls.map((c) => [c.job, c.model.split('.').pop(), c.before, c.fields === undefined ? null : [...c.fields]]),
    [
      ['anonymize', 'Guest', at(30), ['name', 'phone']],
      ['delete', 'Invoice', at(365), null],
      ['anonymize', 'Staff', at(90), ['name', 'badge']],
    ],
  );
  // The report: what each entry changed, in order.
  assert.deepEqual(
    report.map((r) => [r.model.split('.').pop(), r.then, r.before.toISOString(), r.changed, 'error' in r]),
    [
      ['Guest', 'anonymize', at(30), 2, false],
      ['Invoice', 'delete', at(365), 3, false],
      ['Staff', 'anonymize', at(90), 2, false],
    ],
  );

  // The clock is the system's when no `now` is given.
  ({ jobs, calls } = recording());
  const started = Date.now();
  await srv.runApiRetention(jobs);
  const finished = Date.now();
  for (const [c, entry] of calls.map((c, i) => [c, srv.retentionPlan[i]] as const)) {
    assert.ok(entry !== undefined);
    const before = Date.parse(c.before);
    assert.ok(before >= started - entry.afterMs && before <= finished - entry.afterMs, `${c.model} is due before now - afterMs`);
  }

  // An entry that fails does not stop the others, and is in the report: a job that throws and one
  // that rejects.
  ({ jobs, calls } = recording({
    delete: () => {
      throw new Error('the delete failed');
    },
  }));
  report = await srv.runApiRetention(jobs, { now: NOW });
  assert.equal(calls.length, 2, 'the other entries ran');
  assert.deepEqual(report.map((r) => r.changed), [2, 0, 2]);
  assert.ok(report[1]?.error instanceof Error && report[1].error.message === 'the delete failed');
  assert.equal(report[0]?.error, undefined);
  ({ jobs, calls } = recording({
    anonymize: async () => {
      throw new Error('the anonymize failed');
    },
  }));
  report = await srv.runApiRetention(jobs, { now: NOW });
  assert.deepEqual(report.map((r) => r.changed), [0, 3, 0]);
  assert.deepEqual(
    report.map((r) => (r.error instanceof Error ? r.error.message : null)),
    ['the anonymize failed', null, 'the anonymize failed'],
  );
  assert.equal(calls.length, 1, 'the delete entry ran between the failures');
  process.stdout.write('retention ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
