import { CodeBlock, Section, Sub, P, List, Table } from './shared';

export default function RulesTools() {
  return (
    <Section id="rules-tools" title="Rules in veld diff and OpenAPI">
      <Sub id="rules-diff">veld diff</Sub>
      <P>
        <code>veld diff</code> compares business rules as well as types. A rule is identified by its declaration and
        its name, and it counts as changed when its id changes, which happens exactly when its expression changes.
        Expressions are printed back as Veld text, old and new.
      </P>
      <Table
        head={['Change', 'Report']}
        rows={[
          ['Invariant added', 'Breaking where the type is used in requests; a note where it is used only in responses.'],
          ['Invariant removed', 'Breaking where the type is used in responses; non-breaking where it is used only in requests.'],
          ['Invariant expression changed', <strong>possibly breaking</strong>],
          [<><code>require</code> added</>, 'Breaking. Removed: non-breaking. Expression changed: possibly breaking. The else error or its payload changed: breaking.'],
          [<><code>ensure</code> removed</>, 'Breaking. Added: a note. Expression changed: possibly breaking.'],
          [<>Flow removed (also what making a state <code>final</code> does)</>, 'Breaking. Flow added: non-breaking.'],
          [<><code>transition</code> with fewer source states, another target or a new guard</>, 'Breaking. More source states, or the guard removed: non-breaking.'],
          ['Fact added, removed or retyped', 'A note: not a wire change, but the server implementation must change.'],
          ['Message of a rule changed', 'A note.'],
        ]}
      />
      <P>
        Without a solver the tool cannot tell whether a changed expression is tighter or looser, so it reports it
        as <strong>possibly breaking</strong> with both expressions, and never guesses. <code>--strict</code> fails on
        possibly breaking changes too, because passing a tightened rule silently is the worse error. Here the
        limit of a stay changes from 30 days to 14.
      </P>
      <CodeBlock title="Terminal" lang="text">{`$ veld diff --strict
veld 2.0.0-beta.1 · diff
Possibly breaking changes (a rule changed; the direction is unknown) (1) against \`veld.lock.ir.json\`:
  ? bookings:bookings.NewBooking: possibly breaking: rule \`atMost30Days\` changed from \`end - start <= days(30)\` to \`end - start <= days(14)\`; it is checked on requests and responses, and the tool cannot tell whether the new rule is tighter or looser
$ echo $?
1`}</CodeBlock>
      <P>
        Without <code>--strict</code> the same report is printed and the exit code is 0. A declaration that no
        endpoint reaches is treated as used in both requests and responses.
      </P>

      <Sub id="rules-openapi">OpenAPI</Sub>
      <P>
        <code>veld export openapi</code> prints the rules as extensions, so the document doubles as living
        documentation. A contract without rules exports exactly as before. Expressions are printed as Veld text.
      </P>
      <List>
        <li>A model or error schema with invariants has <code>x-veld-rules</code>: a list of <code>{'{name, id, message?, fields, expression}'}</code>. <code>fields</code> are the wire names the rule reads.</li>
        <li>An operation has <code>x-veld-facts</code> (<code>{'{name, type, optional}'}</code>), <code>x-veld-rules</code> with its <code>requires</code> and <code>ensures</code>, and, when it has a transition, <code>x-veld-transition</code> (<code>{'{id, field, from, to}'}</code>).</li>
        <li>An enum that is a state machine has <code>x-veld-flows</code> (<code>{'{from, to}'}</code>) and <code>x-veld-finals</code>.</li>
      </List>
    </Section>
  );
}
