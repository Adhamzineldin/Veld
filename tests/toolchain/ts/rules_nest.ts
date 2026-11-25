// The rules of errors and SSE items through the generated NestJS module: Nest delegates to the same handler as
// the plain adapters, so a declared error and a stream item with rules behave the same (spec/rules.md 5.2).
// Expects `./server` (node, framework nest) generated from tests/toolchain/contract/rules_live.
import 'reflect-metadata';
import assert from 'node:assert/strict';
import type { AddressInfo } from 'node:net';
import { Module } from '@nestjs/common';
import { NestFactory } from '@nestjs/core';
import type { RuleViolationEvent } from './server/runtime/veld';
import { LiveVeldModule } from './server/server/nest/live';
import { impl } from './rules_live_impl';

async function boot(events: RuleViolationEvent[], output?: 'audit'): Promise<{ stop(): Promise<void>; base: string }> {
  const hooks = { ...(output === undefined ? {} : { rules: { output } }), onRuleViolation: (e: RuleViolationEvent) => events.push(e) };
  @Module({ imports: [LiveVeldModule.forRoot({ useValue: impl, hooks })] })
  class AppModule {}
  const app = await NestFactory.create(AppModule, { logger: false, abortOnError: false });
  await app.listen(0, '127.0.0.1');
  const { port } = app.getHttpServer().address() as AddressInfo;
  return { base: `http://127.0.0.1:${String(port)}`, stop: () => app.close() };
}

async function main(): Promise<void> {
  const events: RuleViolationEvent[] = [];
  const strict = await boot(events);
  // A declared error that breaks its own rule is a 500 without detail; a valid one is the typed 409.
  assert.equal((await fetch(`${strict.base}/live/seats?ok=true`)).status, 409);
  const bad = await fetch(`${strict.base}/live/seats?ok=false`);
  assert.equal(bad.status, 500);
  assert.ok(!(await bad.text()).includes('overflow'));
  assert.equal(events.length, 1);
  assert.equal(events[0]?.outcome, 'rejected');
  // A violating SSE item is not sent: the stream ends with the final error event.
  const sse = await (await fetch(`${strict.base}/live/readings?bad=1`)).text();
  assert.ok(sse.includes('"low":0') && sse.includes('event: error') && !sse.includes('"low":5'), sse);
  assert.equal(events.length, 2);
  await strict.stop();

  events.length = 0;
  const audit = await boot(events, 'audit');
  const sent = await (await fetch(`${audit.base}/live/readings?bad=1`)).text();
  assert.ok(sent.includes('"low":5') && !sent.includes('event: error'), sent);
  assert.equal(events[0]?.outcome, 'audited');
  await audit.stop();
  console.log('rules nest ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
