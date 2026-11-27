// The implementation of the rules_live contract (tests/toolchain/contract/rules_live): every endpoint can
// be told to break an invariant of what it sends.
import { Overbooked } from './server/index';
import type { LiveService } from './server/index';

/** The texts of the notes the server accepted. */
export const received: string[] = [];

export const impl: LiveService = {
  async *readings(req) {
    for (let i = 0; i < 3; i += 1) yield req.bad === i ? { low: 5, high: 1 } : { low: i, high: i + 1 };
  },
  async notes(_req, _ctx, ws) {
    for await (const m of ws.incoming) {
      received.push(m.text);
      ws.send(m.text === 'bad' ? { low: 9, high: 1 } : { low: 0, high: m.stars });
    }
  },
  async seats(req) {
    throw new Overbooked({ seats: req.ok ? 12 : 3, limit: 10 });
  },
};
