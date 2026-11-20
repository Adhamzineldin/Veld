// Wire goldens: tests/wire/*.json, checked through the generated runtime and companions.
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import * as A from './copy_a/index';

interface Golden {
  name: string;
  kind: 'json' | 'model';
  decl?: string;
  input: unknown;
  canonical: string;
}

interface Companionish {
  fromWire(wire: unknown): unknown;
  canonical(value: never): string;
  validate(value: unknown): unknown[];
}

const dir = process.env['WIRE_DIR'];
assert.ok(dir, 'WIRE_DIR is not set');
const registry = A as unknown as Record<string, Companionish | undefined>;

let checked = 0;
for (const file of readdirSync(dir).filter((f) => f.endsWith('.json')).sort()) {
  const cases = JSON.parse(readFileSync(join(dir, file), 'utf8')) as Golden[];
  for (const c of cases) {
    if (c.kind === 'json') {
      assert.equal(A.canonical(c.input), c.canonical, `${file}: ${c.name}`);
    } else {
      const companion = registry[c.decl ?? ''];
      assert.ok(companion, `${file}: no companion ${c.decl ?? ''}`);
      const value = companion.fromWire(c.input) as never;
      assert.equal(companion.validate(value).length, 0, `${file}: ${c.name} validates`);
      assert.equal(companion.canonical(value), c.canonical, `${file}: ${c.name}`);
    }
    checked += 1;
  }
}
assert.ok(checked > 0, 'no goldens found');
process.stdout.write(`wire ok (${checked} cases)\n`);
