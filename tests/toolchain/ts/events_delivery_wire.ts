// The dead-letter and retry goldens of phase E2 (tests/wire/events/dead-letter.json and
// retry.json, spec/events.md sections 10.4 and 10.5) through the generated events runtime.
//
// `cases.json` holds the two goldens as they are, the event under test (`BookingPaid` of
// tests/events/bookings.veld, with its rules digest), the topic and the group, and the three
// messages the retry cases need: one that every step accepts, one that step 3 rejects, one that
// step 4 rejects. Prints one JSON object: the copy of every dead-letter case and the settlement of
// every retry case. The Rust test compares them with the goldens.
import { readFileSync } from 'fs';

import { BookingPaid } from './server/events/t';
import type { ConsumerOptions, DeadLetterReason, EventDef, Outcome, Transport, TransportMessage } from './server/runtime/events';
import { deadLetterCopy, subscribeEvents } from './server/runtime/events';

interface DeadCase {
  name: string;
  original?: string;
  originalHex?: string;
  reason: DeadLetterReason;
  group: string;
  attempts: number;
  issues: { path: string; code: string; message: string }[];
  violations: { rule: string; path: string; fields: string[]; message: string; error?: 'overflow' | 'division_by_zero' | 'non_finite' }[];
}

interface RetryCase {
  name: string;
  retry: { max: number; backoff: 'fixed' | 'exponential' } | null;
  deadLetter: boolean;
  delayMs?: number;
  maxDelayMs?: number;
  transportAttempt?: number;
  results?: ('returned' | 'failed')[];
  rejected?: 'malformed' | 'shape' | 'rule';
  ignored?: boolean;
}

interface Input {
  digest: string;
  topic: string;
  group: string;
  messages: { valid: string; shape: string; rule: string; ignored: string; malformed: string };
  deadLetter: DeadCase[];
  retry: RetryCase[];
}

const input = JSON.parse(readFileSync('cases.json', 'utf8')) as Input;
const encoder = new TextEncoder();

const fromHex = (hex: string): Uint8Array => Uint8Array.from((hex.match(/../g) ?? []).map((b) => parseInt(b, 16)));
const toHex = (bytes: Uint8Array): string => Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');

/** A transport that keeps what it is given, and fails the publish of a dead-letter copy when told to. */
class Recording implements Transport {
  deliver: ((m: TransportMessage) => Promise<Outcome>) | undefined;
  published: { topic: string; key: string | null; bytes: Uint8Array }[] = [];
  failCopies = false;

  publish(topic: string, key: string | null, bytes: Uint8Array): Promise<void> {
    if (this.failCopies) return Promise.reject(new Error('the broker refused the copy'));
    this.published.push({ topic, key, bytes });
    return Promise.resolve();
  }

  subscribe(_topic: string, _group: string, deliver: (m: TransportMessage) => Promise<Outcome>): Promise<{ close(): Promise<void> }> {
    this.deliver = deliver;
    return Promise.resolve({ close: () => Promise.resolve() });
  }
}

function messageOf(c: RetryCase): string {
  if (c.ignored === true) return input.messages.ignored;
  switch (c.rejected) {
    case 'malformed':
      return input.messages.malformed;
    case 'shape':
      return input.messages.shape;
    case 'rule':
      return input.messages.rule;
    default:
      return input.messages.valid;
  }
}

/** One delivery of one message to a consumer built the way the golden says. */
async function settle(c: RetryCase, failCopies: boolean): Promise<{
  runs: number;
  failures: { attempt: number; last: boolean }[];
  waitsMs: number[];
  copy: { reason: string; attempts: number; key: string | null; topic: string; hex: string } | null;
  answer: Outcome;
}> {
  const def: EventDef = { name: 'BookingPaid', sig: BookingPaid.sig, schema: BookingPaid.schema, digest: input.digest };
  const transport = new Recording();
  transport.failCopies = failCopies;
  const results = c.results ?? [];
  let runs = 0;
  const failures: { attempt: number; last: boolean }[] = [];
  const waitsMs: number[] = [];
  const options: ConsumerOptions = {
    sleep: (ms) => {
      waitsMs.push(ms);
      return Promise.resolve();
    },
    onHandlerError: (e) => {
      failures.push({ attempt: e.attempt, last: e.final });
    },
  };
  if (c.delayMs !== undefined) options.retryDelayMs = c.delayMs;
  if (c.maxDelayMs !== undefined) options.retryMaxDelayMs = c.maxDelayMs;
  await subscribeEvents(
    transport,
    [
      {
        topic: input.topic,
        group: input.group,
        on: [
          {
            def,
            run: () => {
              runs += 1;
              const result = results[Math.min(runs, results.length) - 1] ?? 'returned';
              if (result === 'failed') throw new Error('the handler failed');
            },
          },
        ],
        ...(c.retry === null ? {} : { retry: c.retry }),
        ...(c.deadLetter ? { deadLetter: true } : {}),
      },
    ],
    options,
  );
  if (transport.deliver === undefined) throw new Error('the consumer did not subscribe');
  const answer = await transport.deliver({ key: 'k1', bytes: encoder.encode(messageOf(c)), attempt: c.transportAttempt ?? 1 });
  const first = transport.published[0];
  let copy = null;
  if (first !== undefined) {
    const parsed = JSON.parse(new TextDecoder().decode(first.bytes)) as Record<string, unknown>;
    copy = {
      reason: String(parsed['veldreason']),
      attempts: Number(parsed['veldattempts']),
      key: first.key,
      topic: first.topic,
      hex: toHex(first.bytes),
    };
  }
  if (transport.published.length > 1) throw new Error(`${c.name}: more than one copy`);
  return { runs, failures, waitsMs, copy, answer };
}

async function main(): Promise<void> {
  const deadLetter = input.deadLetter.map((c) => {
    const original = c.originalHex !== undefined ? fromHex(c.originalHex) : encoder.encode(c.original ?? '');
    const copy = deadLetterCopy(original, c.reason, c.group, c.attempts, c.issues, c.violations);
    return { name: c.name, hex: toHex(copy) };
  });
  const retry = [];
  for (const c of input.retry) {
    const settled = await settle(c, false);
    const failed = await settle(c, true);
    retry.push({ name: c.name, ...settled, answerIfCopyFails: failed.answer, copyFailsRuns: failed.runs });
  }
  console.log(JSON.stringify({ deadLetter, retry }));
}

void main();
