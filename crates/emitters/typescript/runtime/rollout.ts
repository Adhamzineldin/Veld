// Audited rules (`@rollout(audit)`, spec/rules.md section 17.1).
//
// An audited rule is evaluated and reported, and never rejects. The check function of a
// declaration that has one reports it to the sink of this module, and only a server opens a sink:
// a client never does, so a client skips audited rules in the local check and in the partial check.
//
// Rule checks run synchronously, so one module-level sink is safe: it is opened right before a
// walk and closed right after it, with no `await` in between.

import type { Violation } from './veld';

let sink: Violation[] | null = null;
let stashed: { violations: Violation[]; started: number } = { violations: [], started: 0 };

/** The array audited rules report to, or `null` when nobody listens (a client). */
export function auditSink(): Violation[] | null {
  return sink;
}

/** Opens a sink: the rule checks that run next report their audited violations to it. */
export function beginAudit(): void {
  sink = [];
}

/** Closes the sink and returns what it collected. */
export function endAudit(): Violation[] {
  const out = sink ?? [];
  sink = null;
  return out;
}

/** Keeps what a scan found until its caller, which knows the endpoint, reports it. */
export function stashAudited(violations: Violation[], started: number): void {
  stashed = { violations, started };
}

/** What the last scan stashed; the stash is empty afterwards. */
export function takeStashed(): { violations: Violation[]; started: number } {
  const out = stashed;
  stashed = { violations: [], started: 0 };
  return out;
}
