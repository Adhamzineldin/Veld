// Fails if v1-only syntax appears in website/src. Run with `npm run check:docs`.
// `src/pages/docs/Migrating.tsx` is the one place that may show v1 syntax, because it is the
// "Migrating from v1" page (its v1 snippets are labelled as v1).
// `cliHelp.ts` is the verbatim `veld --help` output, which itself names veld.config.json.
// Note: `veld setup` is a v2 command (see spec/config.md), so it is not a v1 token.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'src');
const allowed = new Set(['src/pages/docs/Migrating.tsx', 'src/pages/docs/cliHelp.ts']);

const patterns = [
  ['v1 module block', /^\s*module\s+\w+\s*\{/m],
  ['v1 action block', /^\s*action\s+\w+\s*\{/m],
  ['veld.config.json', /veld\.config\.json/],
  ['backendConfig', /backendConfig/],
  ['@veld/generated', /@veld\/generated/],
  ['v1-only target', /\b(python|csharp|c#|php|dart|flutter|kotlin|swift|angular|svelte)\b/i],
  ['v1 command', /\bveld\s+(validate|watch|ci|docs|graphql|schema|lint)\b/],
  ['go install', /\bgo install\b/],
];

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (/\.(tsx?|css|html)$/.test(name)) yield p;
  }
}

let failures = 0;
for (const file of walk(src)) {
  const rel = relative(root, file).split('\\').join('/');
  if (allowed.has(rel)) continue;
  const text = readFileSync(file, 'utf8');
  for (const [label, re] of patterns) {
    const m = re.exec(text);
    if (m) {
      const line = text.slice(0, m.index).split('\n').length;
      console.error(`${rel}:${line}: v1 token (${label}): ${m[0].trim().slice(0, 60)}`);
      failures += 1;
    }
  }
}
if (failures > 0) {
  console.error(`check:docs failed: ${failures} v1 token(s) in website/src`);
  process.exit(1);
}
console.log('check:docs ok: no v1-only syntax in website/src');
