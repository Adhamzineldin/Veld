import { CodeBlock, Section, Sub, P, Info, Warn, List } from './shared';

export default function RulesTesting() {
  return (
    <Section id="rules-testing" title="Examples and veld test">
      <P>
        An <code>example</code> states what one call does. It is documentation that runs: <code>veld test</code> checks
        it. Examples sit in any <code>.veld</code> file of the project, next to the contract they describe.
      </P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`example "a room that sleeps two refuses three guests" {
  given room = { capacity: 2, closed: false }
  at "2026-06-01T00:00:00Z"
  createBooking(body: {
    roomId: room.id, guests: 3,
    start: "2026-06-10T00:00:00Z", end: "2026-06-12T00:00:00Z"
  }) -> RoomFull { capacity: 2 }
}`}</CodeBlock>
      <List>
        <li><code>given name = value</code> sets a fact the endpoint loads. <code>given name = absent</code> makes the loader find nothing.</li>
        <li>The call names an endpoint and its arguments. <code>at "..."</code> sets the clock. <code>as value</code> sets the principal.</li>
        <li>
          <code>-&gt; ok</code> or <code>-&gt; ErrorName</code> is the outcome. A value after it is a <strong>pattern</strong>: every
          field it names must match, and fields it does not name are ignored.
        </li>
        <li>
          <code>returns value</code> states what the handler returns, so the rules about the result (<code>ensure</code>) can
          run.
        </li>
      </List>
      <P>
        An outcome may be any error the endpoint can answer with, including the implicit ones:{' '}
        <code>ValidationError</code>, <code>NotFound</code>, <code>Unauthorized</code>, <code>Forbidden</code>,{' '}
        <code>InvalidTransition</code> and <code>Internal</code>. The same <code>createBooking</code> example with{' '}
        <code>returns</code> shows an <code>ensure</code> catching a bad handler:
      </P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`example "the handler must keep the guests of the request" {
  given room = { capacity: 4, closed: false }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  at "2026-06-01T00:00:00Z"
  createBooking(body: {
    roomId: room.id, guests: 2,
    start: "2026-06-10T00:00:00Z", end: "2026-06-12T00:00:00Z"
  })
    returns { roomId: room.id, guests: 3, guestId: "00000000-0000-4000-8000-0000000000aa" }
    -> Internal
}`}</CodeBlock>

      <Sub id="rules-example-fill">What an example may leave out</Sub>
      <P>
        Everything you do not write is filled in, the same way every time. A string is <code>a</code> (or as long as{' '}
        <code>@length</code> needs). A number is <code>0</code> moved inside <code>@min</code> and <code>@max</code>. A{' '}
        <code>bool</code> is <code>false</code>. Ids count up from <code>00000000-0000-4000-8000-000000000001</code>.
        Datetimes start at the example's clock. An optional field stays absent. An enum is its first variant. A
        field with <code>@example</code> or a default takes that value.
      </P>
      <P>
        A filled-in value must be valid. If it breaks an invariant, or cannot be filled in at all (a{' '}
        <code>@pattern</code> field with no <code>@example</code>), the example is wrong. <code>veld check</code> reports{' '}
        <code>E0953</code> at the example and names the fields to write.
      </P>

      <Sub id="rules-test-offline">veld test</Sub>
      <P>
        Offline, each example runs against the reference evaluator: no server, no network. One line per example (
        <code>ok</code>, <code>skip</code> with the reason, or <code>FAIL</code> with the expected and the actual outcome), then a
        summary. The exit code is 1 when an example failed. An example that expects <code>ok {'{ ... }'}</code> but has
        no <code>returns</code> cannot be checked offline and is skipped.
      </P>
      <CodeBlock title="Terminal" lang="text">{`$ veld test
veld 2.0.0-beta.1 · test  bookings
ok   a valid booking is accepted
ok   a room that sleeps two refuses three guests
ok   a closed room is refused, whatever the party
ok   a stay cannot end before it starts
...
✓ 38 passed`}</CodeBlock>
      <CodeBlock title="Choosing examples" lang="bash">{`veld test                          # every example, offline
veld test --filter createBooking   # examples whose name or Service.endpoint contains the text
veld test contracts/bookings.veld  # only the examples written in these files`}</CodeBlock>

      <Sub id="rules-test-live">veld test --live</Sub>
      <P>
        <code>--live</code> sends each example as a real HTTP request to a running <strong>test handler</strong> and
        compares the status, the error type and the patterns. The request carries the example's facts, principal
        and clock in the headers <code>Veld-Test-Facts</code>, <code>Veld-Test-Principal</code> and{' '}
        <code>Veld-Test-Now</code>, so the server needs no data of its own.
      </P>
      <P>
        An example writes down only part of an answer: <code>-&gt; RoomFull {'{ capacity: 2 }'}</code> says nothing about
        the violations. So when the answer agrees with the example, <code>veld test</code> also asks the reference
        evaluator what the whole error must be, and the server has to give exactly that: the same type, title,
        status, operation and fields, sent as <code>application/problem+json</code>, and the same violations in the same
        order. A server that answers the right error with the wrong violation fails the example, and the report
        says what the evaluator expected.
      </P>
      <CodeBlock title="Terminal" lang="bash">{`veld test --live http://localhost:3000
veld test --live http://localhost:3000 --dry-run`}</CodeBlock>
      <P>
        An example without <code>returns</code> runs the application's real handler, so <code>-&gt; ok {'{ ... }'}</code>{' '}
        checks what it answered. With <code>--dry-run</code> such an example stops before the handler: the test
        handler answers <code>204</code> with <code>Veld-Test-Passed: 1</code> when every check passed. That makes a live run
        check exactly what an offline run checks, against the generated code.
      </P>
      <P>
        Before the first example, <code>veld test</code> asks <code>GET /__veld/test</code>. A test handler answers that it
        is one and names the services it serves. A server that does not is refused, so a production server is never
        sent test headers. Examples of a service the handler does not name are skipped. In a project with several
        services, run <code>veld test --live</code> once per test handler.
      </P>

      <Sub id="rules-test-handler">The test handler</Sub>
      <P>
        The test handler is generated only for a server target with <code>testing = true</code>, in a file of its own.
        It is available for the <code>node</code>, <code>java</code> and <code>rust</code> server emitters. Only a service with
        endpoint rules gets one.
      </P>
      <CodeBlock title="examples/bookings/veld.toml (excerpt)" lang="toml">{`[[targets]]
name = "api"
role = "server"
emitter = "node"
framework = "express"
service = "bookings"
out = "server/src/generated"

[targets.options]
testing = true`}</CodeBlock>
      <Warn>
        <strong>The test handler must never be mounted in production.</strong> It lets any caller choose the facts,
        the principal and the clock of a request, which switches authorization off. It says so at the top of its file.
        Run it in CI or on a developer machine, never on a server that real users can reach.
      </Warn>
      <P>
        The test handler takes its facts from a header and never calls your loaders. A real handler that reads a
        fact from its own store (as a conditional write does) finds nothing there. The bookings example solves it
        in <code>server/src/test-server.ts</code>: before a handler that moves a booking runs, it puts the booking the
        example gave into the store.
      </P>

      <Sub id="rules-fuzz">veld test --live --fuzz</Sub>
      <P>
        Examples cover the cases you thought of. <code>--fuzz N</code> covers the ones you did not. After the
        examples, it sends <code>N</code> generated requests to every endpoint, valid and invalid, to the test handler in
        dry-run mode. For each request the reference evaluator works out what the answer must be. Any difference in
        the status, the error type, the error's fields or the violations fails the run with exit code 1. A seed
        makes a run repeatable.
      </P>
      <CodeBlock title="Terminal" lang="bash">{`veld test --live http://localhost:3000 --fuzz 200
veld test --live http://localhost:3000 --fuzz 200 --seed 1234567`}</CodeBlock>
      <List>
        <li>
          The first requests are the <strong>boundaries</strong>: every comparison in a rule, tried exactly at the limit
          and one smallest step on each side (<code>guests</code> at <code>room.capacity</code> and one more, a stay of exactly{' '}
          <code>days(30)</code> and one millisecond longer). Then come random requests.
        </li>
        <li>
          For a policy, the generator makes a caller that is sometimes permitted and sometimes denied. For a state
          machine, it sends one request for every state: each source must get past the guard, every other state must
          answer <code>InvalidTransition</code>.
        </li>
        <li>
          An endpoint is skipped, with the reason, when it takes a string with <code>@pattern</code> or{' '}
          <code>@format</code> and no <code>@example</code>, when it is a stream, and when its service has no endpoint rules.
          <code>N</code> is from 1 to 100000. <code>--fuzz</code> needs <code>--live</code>, and <code>--seed</code> needs{' '}
          <code>--fuzz</code>.
        </li>
      </List>
      <CodeBlock title="What the run prints" lang="text">{`fuzz: 200 requests per endpoint, seed 1234567 (repeat with --seed 1234567)
fuzz Bookings.createBooking: 200 sent (24 boundary): NotFound 12, RoomClosed 9, RoomFull 18, ValidationError 61, ok 100
skip Bookings.upload (a string with @pattern or @format and no @example)`}</CodeBlock>
      <P>
        The count per outcome tells you whether the fuzz reached your rules. If every request of an endpoint ended
        in the same error, the run warns: the generator could not get past that check, so the rules after it were
        not exercised. An <code>@example</code> on the field usually helps.
      </P>
      <P>
        A disagreement prints the seed, the endpoint, what the reference evaluator expects, what the server
        answered, and the request as an <code>example</code> block you can paste into the contract. It passes offline and
        fails live until the server is fixed.
      </P>
      <CodeBlock title="A failing run" lang="text">{`FAIL Bookings.createBooking (seed 1234567, boundary request 1)
     expected: RoomFull 409 {"capacity":372} violations [require: precondition failed]
     actual:   HTTP 204 (passed)
     the request as an example (paste it into a contract; offline it passes):
       example "fuzz seed 1234567 Bookings.createBooking request 1" { ... }`}</CodeBlock>
      <Info>
        Without <code>--seed</code>, the seed comes from the clock and is printed first, so any run can be repeated:
        the same seed and the same contract give the same requests.
      </Info>
    </Section>
  );
}
