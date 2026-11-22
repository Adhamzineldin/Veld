// A hand-written FLAKY server for the reconnect tests: it drops connections on purpose, per scenario.
// SSE: `/live/rooms/<scenario>-<id>/events`; WebSocket: `/live/chat/<scenario>-<id>`. The scenarios are
// `resume` (the first connection breaks after some items), `always` (every connection breaks),
// `queue` (WebSocket: the first connection breaks at once, then it echoes), `missing` and `nosuch` (a
// declared error) and `ok`. `GET /__seen?key=<key>` lists the connections of a scenario.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { WebSocketServer } from 'ws';
import type { WebSocket as ServerSocket } from 'ws';
import * as cli from './client/index';

const tick = (n: number): string => JSON.stringify({ label: `tick ${n}`, n, when: '2024-05-01T12:00:00Z' });
const problem = (): string =>
  JSON.stringify({ type: cli.NoSuchRoom.sig, title: 'NoSuchRoom', status: 404, operation: null, data: { room: 'x' } });

/** Connections seen per scenario, with their time and (SSE) `Last-Event-ID`. */
const log = new Map<string, { at: number; lastEventId: string | undefined }[]>();
export const conn = (room: string, lastEventId: string | undefined): number => {
  const list = log.get(room) ?? [];
  list.push({ at: Date.now(), lastEventId });
  log.set(room, list);
  return list.length;
};
export const seen = (room: string): { at: number; lastEventId: string | undefined }[] => log.get(room) ?? [];

function sse(room: string, req: http.IncomingMessage, res: http.ServerResponse): void {
  const lastId = req.headers['last-event-id'];
  const n = conn(room, typeof lastId === 'string' ? lastId : undefined);
  const kind = room.split('-')[0];
  if (kind === 'missing') {
    res.writeHead(404, { 'content-type': 'application/problem+json' });
    res.end(problem());
    return;
  }
  res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' });
  const frame = (i: number): void => void res.write(`id: ${i}\ndata: ${tick(i)}\n\n`);
  if (kind === 'always') {
    res.flushHeaders();
    setTimeout(() => res.socket?.destroy(), 10);
  } else if (kind === 'resume' && n === 1) {
    frame(0);
    frame(1);
    setTimeout(() => res.socket?.destroy(), 20);
  } else if (kind === 'resume') {
    // Resumes after the id the client reports.
    for (let i = Number(lastId) + 1; i <= Number(lastId) + 2; i += 1) frame(i);
    res.end();
  } else {
    frame(0);
    frame(1);
    res.end();
  }
}

function ws(room: string, socket: ServerSocket): void {
  const n = conn(`ws:${room}`, undefined);
  const kind = room.split('-')[0];
  const heard = (seq: number, text: string): string => JSON.stringify({ data: { from: 'srv', seq, text } });
  if (kind === 'always' || (kind === 'queue' && n === 1)) {
    socket.terminate();
  } else if (kind === 'nosuch') {
    socket.send(JSON.stringify({ error: JSON.parse(problem()) as unknown }));
    socket.close(1008);
  } else if (kind === 'resume' && n === 1) {
    socket.send(heard(0, 'a'));
    // Drop once the client has answered a ping sent after the frame, so frame 0 was received
    // first; a fixed timer races the client's scheduling on a loaded machine.
    const drop = (): void => socket.terminate();
    socket.once('pong', drop);
    socket.ping();
    setTimeout(drop, 2000);
  } else if (kind === 'resume') {
    socket.send(heard(1, 'b'));
    socket.send(heard(2, 'c'));
    setTimeout(() => socket.close(1000), 20);
  } else if (kind === 'queue') {
    socket.on('message', (data) => {
      const text = (JSON.parse(data.toString()) as { text: string }).text;
      socket.send(heard(0, text));
      setTimeout(() => socket.close(1000), 20);
    });
  }
}

export function serve(port = 0): Promise<{ server: http.Server; base: string }> {
  const server = http.createServer((req, res) => {
    const m = /^\/live\/rooms\/([^/]+)\/events$/.exec(req.url?.split('?')[0] ?? '');
    if (m?.[1]) sse(m[1], req, res);
    else if (req.url?.startsWith('/__seen?key=')) {
      // The connections the server saw, for tests in other processes (the Java client).
      const key = decodeURIComponent(req.url.slice('/__seen?key='.length));
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify(seen(key)));
    } else res.writeHead(404).end();
  });
  const sockets = new WebSocketServer({ server });
  sockets.on('connection', (socket, req) => {
    const m = /^\/live\/chat\/([^/?]+)/.exec(req.url ?? '');
    if (m?.[1]) ws(m[1], socket);
  });
  return new Promise((resolve) => {
    server.listen(port, '127.0.0.1', () => {
      resolve({ server, base: `http://127.0.0.1:${String((server.address() as AddressInfo).port)}` });
    });
  });
}

