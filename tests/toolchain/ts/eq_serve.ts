// One generated Node server for the HTTP equality tests: `eq_serve.js alpha|beta`. Every `fetch`
// raises `Gone`. Prints `PORT=<n>` and serves until stdin closes.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as alphaSrv from './srv_alpha/index';
import * as betaSrv from './srv_beta/index';

const which = process.argv[2];
const listener =
  which === 'alpha'
    ? alphaSrv.toNodeHttp(
        alphaSrv.createAlphaHandler({
          async fetch(req) {
            throw new alphaSrv.Gone({ id: req.id });
          },
          async item(req) {
            return { id: req.id };
          },
          async thing(req) {
            return { id: req.id };
          },
          async echo(req) {
            return { id: req.body.id };
          },
        }),
      )
    : betaSrv.toNodeHttp(
        betaSrv.createBetaHandler({
          async fetch(req) {
            throw new betaSrv.Gone({ id: req.id });
          },
        }),
      );
const server = http.createServer(listener);
server.listen(0, '127.0.0.1', () => {
  process.stdout.write(`PORT=${String((server.address() as AddressInfo).port)}\n`);
});
process.stdin.on('end', () => process.exit(0));
process.stdin.resume();
