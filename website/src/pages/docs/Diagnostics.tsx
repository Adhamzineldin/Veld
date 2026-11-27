import type { ReactNode } from 'react';
import { Section, P, Table } from './shared';
import diagnosticsMd from '../../../../spec/diagnostics.md?raw';

interface Row {
  code: string;
  kind: string;
  by: string;
  meaning: string;
}

/** Parses the table of spec/diagnostics.md. That file is generated from the compiler, so this never drifts. */
function parseDiagnostics(md: string): Row[] {
  const rows: Row[] = [];
  for (const line of md.split(/\r?\n/)) {
    const m = /^\|\s*`([ELW]\d{4})`\s*\|\s*(\w+)\s*\|\s*([^|]*?)\s*\|\s*(.*?)\s*\|\s*$/.exec(line);
    if (m) rows.push({ code: m[1], kind: m[2], by: m[3], meaning: m[4] });
  }
  return rows;
}

/** Renders `code` spans in a meaning as <code>. */
function inline(text: string): ReactNode[] {
  return text.split(/(`[^`]+`)/g).map((part, i) =>
    part.startsWith('`') && part.endsWith('`') ? <code key={i}>{part.slice(1, -1)}</code> : part,
  );
}

const diagnostics = parseDiagnostics(diagnosticsMd);

export default function Diagnostics() {
  return (
    <Section id="diagnostics" title="Diagnostics">
      <P>
        Every diagnostic has a stable code: <code>E####</code> is an error, <code>W####</code> a warning and{' '}
        <code>L####</code> a lint. Codes are never reused. The ranges are <code>E00xx</code> syntax,{' '}
        <code>E01xx</code> modules, <code>E02xx</code> names, <code>E03xx</code> types, <code>E04xx</code> declarations,{' '}
        <code>E05xx</code> services, <code>E06xx</code> annotations, <code>E07xx</code> workspace and config,{' '}
        <code>E08xx</code> generation, and <code>E09xx</code> business rules (expressions, endpoint rules, state
        machines, policies and examples, with the warnings <code>W09xx</code>). The lint <code>L0014</code>,{' '}
        <code>auth-required</code>, warns about an endpoint with no <code>@auth</code>; write <code>@auth(none)</code> when it is
        public. This table is rendered at build time from{' '}
        <code>spec/diagnostics.md</code>, which is generated from the compiler ({diagnostics.length} codes).
      </P>
      <Table
        head={['Code', 'Kind', 'Reported by', 'Meaning']}
        rows={diagnostics.map((d) => [<code id={d.code}>{d.code}</code>, d.kind, d.by, inline(d.meaning)])}
      />
    </Section>
  );
}
