// Flags over the production handler and the client (spec/platform.md section 11,
// spec/targets/typescript.md section 5j). Expects `./server` and `./client` generated from
// tests/toolchain/contract/platform_p2/flags.veld.
import assert from 'node:assert/strict';
import * as cli from './client/index';
import * as srv from './server/index';
import type { VeldRequest } from './server/runtime/veld';

const ID = '00000000-0000-4000-8000-000000000001';
const rooms = new Map<string, srv.Room>();
const handled: string[] = [];
const errors: unknown[] = [];

const impl: srv.RoomsService = {
  async bookRoom(req) {
    handled.push('bookRoom');
    return req.room;
  },
  async publishRoom(req) {
    handled.push('publishRoom');
    return { ...req.room, phase: srv.Phase.live };
  },
  async archiveRoom(req) {
    handled.push('archiveRoom');
    return { ...req.room, phase: srv.Phase.archived };
  },
};

const roles: Record<string, string[]> = {
  staff: ['staff'],
  beta: ['beta'],
  nobody: [],
  suspended: ['staff', 'suspended'],
};

/** A provider that counts what it is asked, and answers from `on`, or fails. */
class Counting implements srv.veld.FlagProvider {
  readonly asked: { name: string; context: srv.veld.FlagContext }[] = [];
  failing = false;
  constructor(readonly on: string[] = []) {}
  isOn(name: string, context: srv.veld.FlagContext): boolean {
    this.asked.push({ name, context });
    if (this.failing) throw new Error('the flag provider failed');
    return this.on.includes(name);
  }
  count(name: string): number {
    return this.asked.filter((a) => a.name === name).length;
  }
}

function handlerFor(provider: srv.veld.FlagProvider): srv.veld.VeldHandler {
  return srv.createRoomsHandler(impl, {
    facts: { async room(key) { return rooms.get(key); } },
    flags: provider,
    auth: { bearer: async (token) => (roles[token] === undefined ? null : { id: ID, roles: roles[token] ?? [] }) },
    onError: (err) => errors.push(err),
  });
}

function post(path: string, token: string): VeldRequest {
  return { method: 'POST', path, headers: { authorization: `Bearer ${token}` } };
}

function put(open: boolean, phase: srv.Phase = srv.Phase.draft, capacity = 2): void {
  rooms.set(ID, { id: ID, open, capacity, phase });
  handled.length = 0;
}

interface Doc {
  type: string;
  data: Record<string, unknown>;
  violations?: { message: string }[];
}

async function answer(handler: srv.veld.VeldHandler, path: string, token: string): Promise<{ status: number; doc: Doc }> {
  const res = await handler(post(path, token));
  return { status: res?.status ?? 0, doc: JSON.parse(res?.body ?? '{}') as Doc };
}

async function main(): Promise<void> {
  // A require behind a flag: off it is not there, on it applies.
  put(false);
  const off = handlerFor(new Counting());
  assert.equal((await answer(off, `/rooms/${ID}/book`, 'staff')).status, 200);
  const on = handlerFor(new Counting(['openOnly']));
  const refused = await answer(on, `/rooms/${ID}/book`, 'staff');
  assert.equal(refused.status, 422);
  assert.equal(refused.doc.type, srv.ValidationError.sig);
  assert.equal(refused.doc.violations?.length, 1);
  put(true);
  assert.equal((await answer(on, `/rooms/${ID}/book`, 'staff')).status, 200);

  // A permit behind a flag grants only while it is on.
  put(true);
  assert.equal((await answer(off, `/rooms/${ID}/book`, 'beta')).status, 403);
  assert.equal((await answer(handlerFor(new Counting(['betaDesk'])), `/rooms/${ID}/book`, 'beta')).status, 200);

  // A forbid behind a flag restricts only while it is on, with its message.
  assert.equal((await answer(off, `/rooms/${ID}/book`, 'suspended')).status, 200);
  const locked = await answer(handlerFor(new Counting(['lockSuspended'])), `/rooms/${ID}/book`, 'suspended');
  assert.equal(locked.status, 403);
  assert.equal(locked.doc.data['message'], 'suspended');

  // A failing provider: what restricts applies, what grants does not, and each failure goes to onError.
  const failing = new Counting();
  failing.failing = true;
  const broken = handlerFor(failing);
  put(false);
  errors.length = 0;
  assert.equal((await answer(broken, `/rooms/${ID}/book`, 'suspended')).status, 403, 'the forbid counts as on');
  assert.equal((await answer(broken, `/rooms/${ID}/book`, 'beta')).status, 403, 'the permit counts as off');
  assert.ok(errors.length >= 3, 'the failures went to onError');
  // The require of a closed room counts as on.
  assert.equal((await answer(broken, `/rooms/${ID}/book`, 'staff')).status, 422, 'the require counts as on');

  // One reading per request and flag, whatever the number of statements that have it.
  const counting = new Counting(['openOnly']);
  const counted = handlerFor(counting);
  put(true);
  await answer(counted, `/rooms/${ID}/book`, 'staff');
  for (const name of ['openOnly', 'betaDesk', 'lockSuspended']) assert.equal(counting.count(name), 1, name);
  assert.equal(counting.asked[0]?.context.operation.startsWith('veld:'), true);
  assert.equal(counting.asked[0]?.context.tenant, undefined);
  assert.deepEqual(counting.asked[0]?.context.principal, { id: ID, roles: ['staff'] });
  await answer(counted, `/rooms/${ID}/book`, 'staff');
  assert.equal(counting.count('openOnly'), 2, 'a second request reads again');
  // A request refused before the flags are read does not read them.
  const before = counting.asked.length;
  assert.equal((await counted(post(`/rooms/${ID}/book`, 'unknown')))?.status, 401);
  assert.equal(counting.asked.length, before);

  // A flagged edge: off it is not an edge for the guard.
  put(true, srv.Phase.live);
  assert.equal((await answer(off, `/rooms/${ID}/archive`, 'staff')).status, 409);
  assert.equal((await answer(handlerFor(new Counting(['archiving'])), `/rooms/${ID}/archive`, 'staff')).status, 200);
  assert.equal((await answer(broken, `/rooms/${ID}/archive`, 'staff')).status, 409, 'a failing provider counts a flow as off');
  // An ordinary edge does not depend on any flag.
  put(true, srv.Phase.draft);
  assert.equal((await answer(off, `/rooms/${ID}/publish`, 'staff')).status, 200);

  // A server created without the provider does not start.
  assert.throws(
    () => srv.createRoomsHandler(impl, { facts: { async room() { return undefined; } }, auth: { bearer: async () => null } } as unknown as srv.RoomsHooks),
    /hooks\.flags is required/,
  );
  // The memory provider.
  const memory = new srv.veld.MemoryFlagProvider(['a']);
  assert.equal(memory.isOn('a'), true);
  memory.set('a', false);
  assert.equal(memory.isOn('a'), false);

  // The client: `can<Endpoint>` and the policy function, with and without the answers.
  const room: cli.Room = { id: ID, open: true, capacity: 2, phase: 'live' };
  const staff: cli.User = { id: ID, roles: ['staff'] };
  const beta: cli.User = { id: ID, roles: ['beta'] };
  const suspended: cli.User = { id: ID, roles: ['staff', 'suspended'] };
  const flags = (...names: string[]) => (name: string): boolean => names.includes(name);
  assert.equal(cli.RoomsClient.canArchiveRoom(room, staff), false, 'without the function a flow counts as off');
  assert.equal(cli.RoomsClient.canArchiveRoom(room, staff, flags('archiving')), true);
  assert.equal(cli.RoomsClient.canArchiveRoom(room, staff, flags()), false);
  assert.equal(cli.RoomsClient.canBookRoom(room, beta), false, 'without the function a permit counts as off');
  assert.equal(cli.RoomsClient.canBookRoom(room, beta, flags('betaDesk')), true);
  assert.equal(cli.RoomsClient.canBookRoom(room, suspended), false, 'without the function a forbid counts as on');
  assert.equal(cli.RoomsClient.canBookRoom(room, suspended, flags()), true);
  assert.equal(cli.RoomsClient.canBookRoom(room, staff), true);
  assert.equal(cli.manage(beta, room), false);
  assert.equal(cli.manage(beta, room, flags('betaDesk')), true);
  // `nextStates` lists the flagged edge too: the server decides.
  assert.deepEqual(cli.Phase.nextStates('live'), ['archived']);
  console.log('flags_http ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
