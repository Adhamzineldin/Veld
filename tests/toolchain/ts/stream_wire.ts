// Wire goldens of the SSE framing: tests/wire/streaming/sse.json, checked through the generated runtime.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { SseParser, sseFrame } from './client/runtime/veld';
import type { SseMessage, SseOptions } from './client/runtime/veld';

interface Frame {
  name: string;
  data: string;
  options: SseOptions;
  frame: string;
}

interface Parse {
  name: string;
  chunks: string[];
  events: { event: string; data: string; id?: string }[];
}

const dir = process.env['WIRE_DIR'];
assert.ok(dir, 'WIRE_DIR is not set');
const golden = JSON.parse(readFileSync(join(dir, 'streaming', 'sse.json'), 'utf8')) as { frames: Frame[]; parse: Parse[] };

for (const f of golden.frames) assert.equal(sseFrame(f.data, f.options), f.frame, f.name);
for (const p of golden.parse) {
  const parser = new SseParser();
  const got: SseMessage[] = [];
  for (const chunk of p.chunks) got.push(...parser.push(chunk));
  const normalised = got.map((m) => (m.id === undefined ? { event: m.event, data: m.data } : { event: m.event, data: m.data, id: m.id }));
  assert.deepEqual(normalised, p.events, p.name);
  // The same bytes fed one character at a time give the same events.
  const slow = new SseParser();
  const slowGot: SseMessage[] = [];
  for (const ch of p.chunks.join('')) slowGot.push(...slow.push(ch));
  assert.deepEqual(
    slowGot.map((m) => (m.id === undefined ? { event: m.event, data: m.data } : { event: m.event, data: m.data, id: m.id })),
    p.events,
    `${p.name} (one character at a time)`,
  );
}
console.log(`sse wire goldens ok: ${golden.frames.length} frames, ${golden.parse.length} parses`);
