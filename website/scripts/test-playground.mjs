// Exercise the shipped website and its WASM worker. The native parity test produces the CLI hashes.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { unzipSync } from 'fflate';
import { chromium } from 'playwright-core';
import { preview } from 'vite';
import ts from 'typescript';

async function typecheckUsage(files, prefix, example) {
  const work = await mkdtemp(join(tmpdir(), 'veld-playground-'));
  try {
    for (const [path, bytes] of Object.entries(files)) {
      const output = join(work, path.slice(prefix.length + 1));
      await mkdir(dirname(output), { recursive: true });
      await writeFile(output, bytes);
    }
    const main = join(work, 'example.ts');
    await writeFile(main, example);
    const pkg = JSON.parse(await readFile(join(work, 'package.json'), 'utf8'));
    const program = ts.createProgram([main], {
      target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler, strict: true, noEmit: true, skipLibCheck: true,
      paths: { [pkg.name]: [join(work, 'index.ts')] }, types: [],
    });
    const errors = ts.getPreEmitDiagnostics(program);
    assert.equal(errors.length, 0, prefix + ': ' + ts.formatDiagnostics(errors, {
      getCanonicalFileName: (name) => name, getCurrentDirectory: () => work, getNewLine: () => '\n',
    }));
  } finally {
    await rm(work, { force: true, recursive: true });
  }
}

const site = fileURLToPath(new URL('../', import.meta.url));
const targetDir = process.env.CARGO_TARGET_DIR ?? resolve(site, '../target');
const cases = JSON.parse(await readFile(resolve(targetDir, 'veld-wasm-parity/cases.json'), 'utf8'));
const server = await preview({ root: site, configFile: false, preview: { host: '127.0.0.1', port: 0 } });
let browser;
try {
  browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 1440, height: 960 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (err) => errors.push(err.message));
  const url = new URL('playground', server.resolvedUrls.local[0]).href;
  await page.goto(url);
  const button = (name) => page.getByRole('button', { name, exact: true });
  const download = button('Download .zip');
  const editor = page.locator('textarea:not([readonly])');
  const output = page.locator('textarea[readonly]');

  // First-visit offline support must include the app shell and WASM worker dependencies.
  await download.click({ trial: true });
  await page.waitForFunction(() => Boolean(navigator.serviceWorker.controller));
  await context.setOffline(true);
  await page.reload();
  await download.click({ trial: true });
  await context.setOffline(false);

  const presets = { hello: 'Hello', errors: 'Errors', 'library-package': 'Shared library', microservices: 'Microservices', streaming: 'Streaming' };
  for (const [id, title] of Object.entries(presets)) {
    const entry = cases.find((c) => c.name.startsWith('preset:' + id + ' / ') && c.target.name === c.target.role + '-' + c.target.emitter);
    assert(entry?.ok, 'Generate fresh CLI parity fixtures for preset ' + id);
    await button(title).click();
    const [archive] = await Promise.all([page.waitForEvent('download'), download.click()]);
    const prefix = 'veld-' + id + '-' + entry.target.emitter + '-' + entry.target.role;
    assert.equal(archive.suggestedFilename(), prefix + '.zip');
    const files = unzipSync(await readFile(await archive.path()));
    const hashes = {};
    for (const [path, bytes] of Object.entries(files)) {
      assert(path.startsWith(prefix + '/'), path);
      hashes[path.slice(prefix.length + 1)] = createHash('sha256').update(bytes).digest('hex');
    }
    assert.deepEqual(hashes, entry.expected, title + ' ZIP differs from CLI output');
    const usage = page.locator('details').filter({ has: page.locator('summary', { hasText: 'Usage example' }) });
    await usage.locator('summary').click();
    const example = await usage.locator('code').innerText();
    if (id === 'streaming') {
      assert.match(example, /live\.ticks\(/);
      assert.match(example, /chat\.send\(/);
    }
    await typecheckUsage(files, prefix, example);
    await usage.locator('summary').click();
  }

  const streaming = cases.filter((c) => c.name.startsWith('preset:streaming / t'));
  assert(streaming.length > 0);
  const labels = { node: 'Node.js', typescript: 'TypeScript', java: 'Java', rust: 'Rust' };
  for (const { target, ok, expected } of streaming) {
    await button(labels[target.emitter] + ' ' + target.role).click();
    if (target.framework) await page.getByLabel('Framework', { exact: true }).selectOption(target.framework);
    if (target.emitter === 'typescript') await page.getByLabel('Query options').setChecked(!!target.options['query-options']);
    if (ok) {
      await page.getByText(Object.keys(expected).length + ' files', { exact: true }).waitFor();
      await download.click({ trial: true });
    } else {
      await page.locator('a[href="/docs#E0801"]').first().waitFor();
      assert(await download.isDisabled(), target.framework);
    }
  }

  await button('Hello').click();
  await output.waitFor();
  const original = await editor.inputValue();
  await editor.fill(original.replace('message: string', 'message: UnknownType'));
  await page.locator('a[href="/docs#E0205"]').first().waitFor();
  assert.equal(await page.locator('summary', { hasText: 'Usage example' }).count(), 0);
  await page.getByRole('button', { name: /^hello\/hello\.veld:3:/ }).click();
  assert(await editor.evaluate((el) => el === document.activeElement));
  await editor.fill(original.replaceAll('Greeting', 'RevisedGreeting'));
  await download.click({ trial: true });
  const guide = page.locator('details').filter({ has: page.locator('summary', { hasText: 'Node.js server guide' }) });
  await guide.locator('summary').click();
  // The generated README is rendered: fenced code becomes code blocks, never literal backticks.
  assert((await guide.locator('pre code').count()) > 0, 'guide code blocks');
  assert.doesNotMatch(await guide.innerText(), /```/);
  await guide.locator('summary').click();
  await editor.fill(original.replace('message: string', 'message:     string'));
  await button('Format').click();
  await page.waitForFunction((text) => document.querySelector('textarea:not([readonly])')?.value === text, original);
  await download.click({ trial: true });
  await page.getByRole('navigation', { name: 'Files' }).getByTitle('README.md', { exact: true }).click();
  assert.match(await output.inputValue(), /Veld/i);
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: new URL(url).origin });
  await button('Copy').click();
  await button('Copied').waitFor();
  // The Windows clipboard turns \n into \r\n; compare the text, not the platform's line endings.
  const pasted = (await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/g, '\n');
  assert.equal(pasted, await output.inputValue());
  // Older share links saved the label "none" as the streaming framework.
  const legacy = cases.find((c) => c.name === 'preset:streaming / client-typescript');
  const snapshot = {
    presetId: 'streaming', files: legacy.files, project: legacy.project,
    choice: { key: 'typescript:client', framework: 'none', service: '', language: '', validate: true, queryOptions: false },
  };
  await page.goto('about:blank');
  await page.goto(url + '#s=' + Buffer.from(JSON.stringify(snapshot)).toString('base64url'));
  await download.click({ trial: true });
  assert.equal(await page.getByLabel('Framework', { exact: true }).inputValue(), '');
  await page.waitForFunction(() => Boolean(navigator.serviceWorker.controller));
  await context.setOffline(true);
  await page.reload();
  await download.click({ trial: true });
  await button('Rust server').click();
  await page.getByText('20 files', { exact: true }).waitFor();
  await download.click({ trial: true });
  assert.deepEqual(errors, []);
  console.log('Chromium: five preset ZIPs match the CLI and all five usage examples typecheck; ' + streaming.length + ' streaming targets, editor, diagnostics, format, clipboard, legacy links and offline generation passed.');
} finally {
  await browser?.close();
  await server.close();
}
