import { CodeBlock, Section, Sub, P, Info, Warn, List, Table } from './shared';

export default function Rules() {
  return (
    <Section id="rules" title="Business rules">
      <P>
        A rule is a statement about your data that must always be true. The contract states it. The
        generated server enforces it and the generated client checks it before it sends a request. You
        write it once, so the server, the web form and the mobile app cannot drift apart.
      </P>
      <P>
        These pages use <code>examples/bookings</code> from the repository: a small hotel-booking service.
        Everything shown here runs. The full language is <code>spec/rules.md</code>.
      </P>
      <Table
        head={['Kind', 'Says', 'Page']}
        rows={[
          [<code>rule</code>, 'A model or an error must always satisfy a condition.', <a href="#rules-invariants">Invariants</a>],
          [<><code>fact</code>, <code>require</code>, <code>ensure</code></>, 'What an endpoint needs to load, may start from and promises to return.', <a href="#rules-endpoint">Endpoint rules</a>],
          [<><code>flow</code>, <code>final</code>, <code>transition</code></>, 'Which state changes are legal, and which one an endpoint makes.', <a href="#rules-states">State machines</a>],
          [<><code>principal</code>, <code>policy</code>, <code>allow</code></>, 'Who may call.', <a href="#rules-policies">Policies</a>],
          [<code>example</code>, <>What one call does. Documentation that <code>veld test</code> checks.</>, <a href="#rules-testing">Examples and veld test</a>],
        ]}
      />
      <P>
        Rules are plain boolean expressions. They have no side effects and always finish, so every
        generator enforces them the same way.
      </P>

      <Sub id="rules-invariants">Invariants: rules about one value</Sub>
      <P>
        An invariant is a <code>rule</code> inside a <code>model</code> or an <code>error</code>. It must hold for every
        value of that type. A constraint such as <code>@min(1)</code> covers one field. A rule can compare
        several fields. Its <code>message</code> is what a person sees; without one the message is{' '}
        <code>violates rule &lt;name&gt;</code>.
      </P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`model NewBooking {
  roomId: uuid
  guests: int @min(1)
  start: datetime
  end: datetime

  rule endAfterStart: end > start
    message "The stay must end after it starts"
  rule atMost30Days: end - start <= days(30)
    message "A stay is at most 30 days"
}`}</CodeBlock>
      <P>
        An expression uses the fields of the value, literals, enum literals such as <code>.paid</code>,
        arithmetic, comparisons, <code>&amp;&amp;</code>, <code>||</code>, <code>not</code>,{' '}
        <code>implies</code> and <code>in</code>. There are a few built-in functions. There is no
        <code>null</code>, no ternary and no loop.
      </P>
      <Table
        head={['Function', 'What it does']}
        rows={[
          [<code>has(path)</code>, 'True when an optional field on the path is present. Reading an optional field is allowed only where it is known present.'],
          [<code>len(x)</code>, 'The length of a string, list, map or bytes.'],
          [<code>matches(s, "re")</code>, <>The same regex subset as <code>@pattern</code>.</>],
          [<><code>startsWith</code>, <code>endsWith</code>, <code>contains</code></>, 'Text tests on two strings.'],
          [<><code>days</code>, <code>hours</code>, <code>minutes</code>, <code>seconds</code>, <code>ms</code></>, <>A duration. A difference of two datetimes is a duration: <code>end - start &lt;= days(30)</code>.</>],
          [<><code>all</code>, <code>any</code>, <code>count</code>, <code>sum</code></>, <>Over a list, with a lambda: <code>all(items, i =&gt; i.quantity &gt; 0)</code>.</>],
        ]}
      />
      <CodeBlock title="Optional fields and lists" lang="veld">{`model NewOrder {
  items: OrderItem[] @length(min: 1, max: 50)
  coupon?: string
  discount?: int @min(0) @max(100)

  rule discountNeedsCoupon: has(discount) implies has(coupon)
    message "A discount needs a coupon"
  rule atMost100Units: sum(items, i => i.quantity) <= 100
    message "An order has at most 100 units"
}`}</CodeBlock>
      <P>
        The new words are contextual: they are keywords only where the grammar expects them. A field, a
        parameter or an endpoint may still be called <code>rule</code>, <code>fact</code> or{' '}
        <code>message</code>. A rule that cannot be evaluated (an overflow, a division by zero) counts as
        violated: rules fail closed.
      </P>

      <Sub id="rules-violations">What happens when a rule is violated</Sub>
      <P>
        <strong>On the server.</strong> The generated handler checks the request body before your code
        runs. A body that breaks a rule gets <code>422 ValidationError</code>. The problem document lists
        the issues as before and adds a <code>violations</code> list. <code>fields</code> names the fields the
        rule reads, so a client can show the message next to them.
      </P>
      <CodeBlock title="The 422 answer" lang="text">{`{ "title": "ValidationError", "status": 422,
  "data": { "issues": [ { "path": "body", "code": "rule", "message": "The stay must end after it starts" } ] },
  "violations": [ { "rule": "veld:bookings/NewBooking.endAfterStart@ada6a4114a0970df",
                    "path": "body", "fields": ["end", "start"],
                    "message": "The stay must end after it starts" } ] }`}</CodeBlock>
      <P>
        <strong>In the client.</strong> Before it sends a request, the generated client runs the same rules
        on the body. A request that breaks one is <strong>not sent</strong>. The call throws a{' '}
        <code>ValidationError</code> with <code>local = true</code> and the <code>violations</code>. A violation that
        comes back from the server has the same <code>violations</code> and <code>local = false</code>. Turn the local
        check off with <code>new BookingsClient({'{ rules: false }'})</code>.
      </P>
      <CodeBlock title="TypeScript client" lang="ts">{`try {
  await client.createBooking({ body });
} catch (e) {
  if (ValidationError.matches(e)) {
    console.log(e.local);       // true: nothing was sent
    console.log(e.violations);  // [{ rule, path, fields, message }]
  }
}`}</CodeBlock>
      <P>
        You can also check a value yourself. Every model that has rules has <code>NewBooking.check(value)</code>,
        which returns the violations, and <code>NewBooking.partial(draft)</code>, which skips a rule that needs a
        field the draft does not have yet. With <code>framework = "react"</code> the generated{' '}
        <code>useFormRules</code> runs <code>partial</code> on what the user has typed and groups the messages by
        field.
      </P>
      <CodeBlock title="examples/bookings/web/src/BookingForm.tsx (excerpt)" lang="ts">{`import { NewBooking } from '@veld/web';
import { useFormRules } from '@veld/web/bookings/react';

const rules = useFormRules(NewBooking, draft);   // draft: what has been typed so far
rules.messages.end;     // ['The stay must end after it starts'] while the dates are wrong
rules.valid;            // true when the draft has no violations yet`}</CodeBlock>
      <Info>
        An incomplete draft is not invalid: until the user picks a departure, <code>endAfterStart</code> has
        nothing to compare and is skipped. Keep the draft in state as a new object on every change.
      </Info>

      <Sub id="rules-stale">When the client and the server disagree</Sub>
      <P>
        A server may change its rules after a client was built. The server sends the digest of its rules in
        a <code>Veld-Rules</code> header. When it differs from the one the client was generated with, the
        client stops its local checks (the server still checks) and calls <code>onContractMismatch</code> once.
        A browser can read this header from another origin only if the server sends{' '}
        <code>Access-Control-Expose-Headers: Veld-Rules</code>. Without it the client still works, but it never
        rejects a request locally.
      </P>
      <CodeBlock title="TypeScript client" lang="ts">{`new BookingsClient({
  baseUrl,
  onContractMismatch: ({ service }) => console.warn(\`The \${service} rules changed. Rebuild the app.\`),
});`}</CodeBlock>

      <Sub id="rules-belong">What belongs in the contract</Sub>
      <P>
        Put a rule in the contract when it is about the data or the call and can be said with the fields you
        have: ranges, relations between fields, "this must exist and be open", a limit that depends on a
        loaded record, a promise about the result. Keep it in code when it needs more: a database query
        beyond loading one record by key, a call to another service, a random or time-dependent decision
        beyond <code>now</code>, or state that changes between requests.
      </P>

      <Sub id="rules-targets">Where rules are enforced</Sub>
      <P>
        The TypeScript client and the Node server, Java (client, JDK server and Spring) and Rust (client and
        axum server) enforce everything on these pages. A generator that cannot enforce a rule, such as a
        plugin, stops with <code>E0801</code>. It never generates code that ignores a rule. A contract with
        no rules generates the same files as before.
      </P>
      <Warn>
        <code>validate = false</code> on a target turns request validation off, including invariants. Policies and
        the state check still run. <code>veld check</code> warns about it with <code>W0904</code>.
      </Warn>

      <Sub id="rules-coming">Coming next</Sub>
      <P>These are designed in <code>spec/rules.md</code> and not built. Do not use them in a contract today.</P>
      <List>
        <li><code>@rollout(audit)</code>, to watch a new rule before it rejects anything. The compiler and <code>veld test</code> know it; the generators do not, so <code>veld generate</code> stops with <code>E0801</code>.</li>
        <li><code>@version</code> and <code>@idempotent</code>, derived fields, and <code>veld check --prove</code>.</li>
        <li><code>veld test --coverage</code> and <code>veld export rules</code>.</li>
        <li>Events (<code>event</code>, <code>channel</code>, <code>publishes</code>, <code>subscribes</code>) in generated code.</li>
      </List>
    </Section>
  );
}
