// The generated Node server for the streaming contract (tests/toolchain/contract/streaming): the
// implementations, and a helper that serves both services over node:http plus the `ws` package.
// Run directly it prints `PORT=<n>` and serves until stdin closes (used by the Java cross test).
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { WebSocketServer } from 'ws';
import * as srv from './server/index';
import { attachWebSocket } from './server/server/ws';

export const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** Server-side observations the tests assert on. */
export const seen = {
  foreverClosed: 0,
  roomEventsClosed: 0,
  chatEnded: 0,
  sink: [] as string[],
  announceDone: 0,
  principals: [] as unknown[],
};

export function tick(n: number): srv.Tick {
  return { n, label: `tick ${n}`, when: '2024-05-01T12:00:00Z' };
}

const bad = { n: 'x', label: 'bad', when: '2024-05-01T12:00:00Z' } as unknown as srv.Tick;

export const live: srv.LiveService = {
  async *ticks(req) {
    if (req.count < 0) throw new srv.QuotaExceeded({ limit: 0 });
    for (let n = 0; n < req.count; n += 1) {
      if (req.failAt === n) throw new srv.QuotaExceeded({ limit: n });
      yield tick(n);
    }
  },
  async *forever() {
    try {
      for (let n = 0; ; n += 1) {
        yield tick(n);
        await sleep(20);
      }
    } finally {
      seen.foreverClosed += 1;
    }
  },
  async *roomEvents(req, ctx) {
    if (req.room === 'nope') throw new srv.NoSuchRoom({ room: req.room });
    if (req.room === 'bad') yield bad;
    if (req.room === 'bad2') {
      yield tick(0);
      yield bad;
    }
    const last = ctx.headers['last-event-id'];
    const from = typeof last === 'string' ? Number(last) + 1 : 0;
    try {
      for (let n = from; n < from + 3; n += 1) yield srv.veld.sseEvent(tick(n), { event: 'tick', id: String(n) });
    } finally {
      seen.roomEventsClosed += 1;
    }
  },
  async chat(req, _ctx, ws) {
    if (req.room === 'locked') throw new srv.NoSuchRoom({ room: req.room });
    let seq = 0;
    try {
      for await (const m of ws.incoming) {
        ws.send({ from: req.nick ?? 'anon', text: m.loud === true ? m.text.toUpperCase() : m.text, seq });
        seq += 1;
      }
    } finally {
      seen.chatEnded += 1;
    }
  },
  async announce(_req, _ctx, ws) {
    for (let n = 0; n < 3; n += 1) ws.send(tick(n));
    seen.announceDone += 1;
  },
  async sink(_req, _ctx, ws) {
    for await (const m of ws.incoming) {
      seen.sink.push(m.text);
      if (seen.sink.length % 2 === 0) return;
    }
  },
};

export const vault: srv.VaultService = {
  async *feed(_req, ctx) {
    seen.principals.push(ctx.principal);
    yield tick(0);
    yield tick(1);
  },
  async echo(_req, ctx, ws) {
    seen.principals.push(ctx.principal);
    let seq = 0;
    for await (const m of ws.incoming) {
      ws.send({ from: 'vault', text: m.text, seq });
      seq += 1;
    }
  },
};

export const vaultHooks: srv.VaultHooks = {
  auth: { bearer: async (token) => (token === 'good' ? { user: 'u1' } : null) },
};

export interface Running {
  url: string;
  close(): Promise<void>;
}

export interface Options {
  validateResponses?: boolean;
}

/** Both services behind one handler: the second answers what the first does not match. */
export function bothHandlers(options: Options = {}): srv.veld.VeldHandler {
  const liveHandler = srv.createLiveHandler(live, { validateResponses: options.validateResponses === true });
  const vaultHandler = srv.createVaultHandler(vault, { ...vaultHooks, validateResponses: options.validateResponses === true });
  return Object.assign(
    async (req: srv.veld.VeldRequest) => (await liveHandler(req)) ?? vaultHandler(req),
    { ws: async (req: srv.veld.VeldRequest, sock: srv.veld.WsSocket) => (await liveHandler.ws(req, sock)) || vaultHandler.ws(req, sock) },
  );
}

/** Serves both services on one node:http server; `ws` endpoints share the port. */
export async function start(options: Options = {}): Promise<Running> {
  const both = bothHandlers(options);
  const server = http.createServer(srv.toNodeHttp(both));
  const wss = new WebSocketServer({ server });
  attachWebSocket(wss, both);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const port = (server.address() as AddressInfo).port;
  return {
    url: `http://127.0.0.1:${port}`,
    close: () =>
      new Promise<void>((resolve) => {
        for (const c of wss.clients) c.terminate();
        server.closeAllConnections();
        server.close(() => resolve());
      }),
  };
}

if (require.main === module) {
  start().then((s) => {
    process.stdout.write(`PORT=${new URL(s.url).port}\n`);
    process.stdin.on('end', () => process.exit(0));
    process.stdin.resume();
  }, (err: unknown) => {
    console.error(err);
    process.exit(1);
  });
}
