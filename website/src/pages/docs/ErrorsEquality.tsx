import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';

export default function ErrorsEquality() {
  return (
    <>
      <Section id="errors" title="Errors: problem+json">
        <P>
          Every error response is RFC 9457 <code>application/problem+json</code>. The body names the error
          declaration and the operation that raised it by signature.
        </P>
        <CodeBlock title="An error on the wire" lang="text">{`{ "type": "veld:orders/OutOfStock@1c0d...", "title": "OutOfStock", "status": 409,
  "operation": "veld:orders/Orders.createOrder@77e1...", "data": { "sku": "A-1" } }`}</CodeBlock>
        <Sub>Server behaviour</Sub>
        <List>
          <li>The adapter catches what your implementation throws.</li>
          <li>A generated error whose signature is declared for that endpoint is answered as problem+json at its status.</li>
          <li>Anything else, including an <em>undeclared</em> generated error, becomes <code>500 Internal</code> with no message. The original goes to <code>onError</code>. Internal details are never leaked.</li>
        </List>
        <Sub>Client behaviour</Sub>
        <List>
          <li>A problem+json <code>type</code> that matches a declared error is thrown as that class, with <code>op</code> and <code>status</code> set. Fall back to <code>status</code> when the type is unknown.</li>
          <li>Any other non-2xx response is a <code>VeldHttpError</code> (<code>status</code>, <code>body</code>).</li>
          <li>A network failure, a body that is not valid JSON, or a response like HTTP 502 from a proxy is a <code>VeldTransportError</code>.</li>
          <li>The client validates the request first and throws a local <code>ValidationError</code> with <code>op</code> set. Turn it off with the <code>validate</code> option.</li>
        </List>
        <CodeBlock title="A generated error class" lang="ts">{`export class OutOfStock extends VeldError<{ sku: string }> {
  static readonly sig = 'veld:orders/OutOfStock@...';
  static readonly defaultStatus = 409;
  static matches(e: unknown): e is OutOfStock;
}`}</CodeBlock>
      </Section>

      <Section id="signatures" title="Signatures">
        <P>
          Every model, enum, union and error carries a <strong>signature</strong> computed once by the
          compiler from its wire shape, and every endpoint carries an operation signature. Every generated
          target embeds them verbatim, and CI compares them byte for byte.
        </P>
        <Table
          head={['Kind', 'Form']}
          rows={[
            ['Declaration', <code>veld:&lt;package&gt;/&lt;Name&gt;@&lt;hash&gt;</code>],
            ['Operation', <code>veld:&lt;package&gt;/&lt;Service&gt;.&lt;endpoint&gt;@&lt;hash&gt;</code>],
          ]}
        />
        <List>
          <li>The hash is the first 16 hex characters of the SHA-256 of the RFC 8785 canonical JSON of the declaration's shape, including every declaration it reaches. Prelude declarations use the package <code>veld</code>; registry packages keep their <code>@org/name</code>.</li>
          <li>The signature covers only what affects the data on the wire: wire names, types, optionality, enum wire values, union discriminators. It does <strong>not</strong> include docs, deprecations, constraints, defaults, examples or source spans. Renaming a field in source while keeping <code>@wire</code> keeps the signature.</li>
          <li>An operation signature hashes the method and the path with every <code>{'{param}'}</code> replaced by <code>{'{}'}</code>.</li>
          <li>Signatures are computed in one place, <code>veld-compiler::signature</code>. No generator computes or reformats one.</li>
        </List>
      </Section>

      <Section id="equality" title="same() and matches()">
        <P>
          Two copies of <code>Orders.NotFound</code> generated into different directories, packages or
          languages are different classes. Veld therefore compares by signature, never by class or
          prototype identity.
        </P>
        <CodeBlock title="The rule (spec/ir.md section 5.4)" lang="text">{`same(a, b) :=
     a.decl_sig == b.decl_sig
  && (a is error  =>  a.op_sig == b.op_sig)
  && canonical(a) == canonical(b)

X.matches(err) := err.decl_sig == X.decl_sig
                  && service_of(err.op_sig) == service_of(X's declaring context)`}</CodeBlock>
        <Table
          head={['Check', 'True when', 'Use it for']}
          rows={[
            [<code>same(a, b)</code>, 'Same declaration, same operation (for errors) and same canonical value bytes', 'Asserting that two values are identical, in any package or language'],
            [<code>X.matches(err)</code>, 'This declaration, raised by this service, from any operation', 'Catching a typed error: the usual check in a catch block'],
          ]}
        />
        <P>
          The canonical bytes of a value are the RFC 8785 JSON of its wire form after decoding, with
          defaults applied, absent optionals omitted, UTC datetimes with <code>Z</code>, lowercase uuids and
          padded base64. Every generator's runtime provides <code>canonical(value)</code>.
        </P>
        <CodeBlock title="TypeScript" lang="ts">{`import { NotFound, same } from '@veld/web';

try {
  await client.getOrder({ id });
} catch (e) {
  if (NotFound.matches(e)) {
    // this declaration, from this service
  }
}

// same(a, b): same declaration, same operation and same canonical bytes
const identical = same(first, second);`}</CodeBlock>
        <CodeBlock title="Java" lang="java">{`try {
  client.getOrder(new GetOrderRequest(id));
} catch (VeldError e) {
  if (NotFound.matches(e)) { /* this declaration */ }
  if (NotFound.matches(e, "orders/Orders")) { /* ... raised by this service */ }
}
// Veld.same(a, b) compares signature, operation and canonical bytes`}</CodeBlock>
        <Info>
          <strong>Across packages and languages.</strong> The same function works between copies of the
          runtime in different directories, because it reads only the brand, the signatures and the
          canonical data. <code>same</code> is false for a different operation, different data, a different
          declaring service with the same error name, or a changed shape. The repository tests this with
          generated TypeScript and Java that call each other: a <code>NotFound</code> raised by the Node{' '}
          <code>accounts</code> service is recognised by the Java service that calls it.
        </Info>
        <P>
          In Java, <code>catch (OutOfStock e)</code> is reliable only inside one generated package. Across
          packages use <code>matches</code> or <code>Veld.same</code>.
        </P>
      </Section>
    </>
  );
}
