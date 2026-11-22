import { CodeBlock, Section, Sub, P, Info, List, Table, NewBadge } from './shared';

export default function Language() {
  return (
    <>
      <Section id="language" title="The contract language">
        <P>
          A <strong>package</strong> is a directory tree of <code>.veld</code> files. A{' '}
          <strong>module</strong> is one file, named by its path relative to the package root
          (<code>contracts/orders/billing/invoices.veld</code> in package <code>orders</code> is the module{' '}
          <code>billing.invoices</code>). Every top-level declaration is visible to any module that imports
          it: there is no <code>export</code> keyword and no private scope. Declaration names are unique
          within a package.
        </P>
        <CodeBlock title="One file, every construct" lang="veld">{`use iam:users.{User}

/// A customer order.
model Order {
  id: uuid @readonly
  customer: User
  items: OrderItem[] @length(min: 1)
  status: Status = .pending
  note?: string @length(max: 500)
}

enum Status { pending, paid, shipped }

error OutOfStock 409 {
  sku: string
}

/// Order management.
@auth(bearer)
service Orders at "/orders" {
  /// Fetch one order.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound

  post "/" createOrder(body: CreateOrder) -> 201 Order | OutOfStock

  get "/" listOrders(query: OrderFilter) -> Page<Order>

  @auth(none)
  get "/health" health() -> 204
}`}</CodeBlock>
        <P>
          Comments are <code>// line</code>, <code>/* block */</code> (not nested) and{' '}
          <code>/// doc</code>. A doc comment is Markdown, attaches to the next declaration, field or
          endpoint, and ends up in the generated code. Reserved words are{' '}
          <code>use as model enum error union service at extends const stream true false null</code>. The
          HTTP methods (<code>get post put patch delete head options ws</code>) are keywords only where a
          method is expected, so a field may still be called <code>get</code> or <code>body</code>.
        </P>
        <Sub>Scalar types</Sub>
        <Table
          head={['Veld', 'Meaning', 'JSON on the wire']}
          rows={[
            [<code>string</code>, 'UTF-8 text', 'string'],
            [<code>int</code>, 'signed 32-bit', 'number'],
            [<code>long</code>, 'signed 64-bit', 'string (to stay exact in JS)'],
            [<code>float</code>, 'IEEE-754 double', 'number'],
            [<code>decimal</code>, 'arbitrary-precision decimal', 'string'],
            [<code>bool</code>, 'boolean', <><code>true</code> / <code>false</code></>],
            [<code>date</code>, 'RFC 3339 full-date', <code>YYYY-MM-DD</code>],
            [<code>datetime</code>, 'RFC 3339 date-time, UTC on the wire', <code>YYYY-MM-DDTHH:MM:SS[.fff]Z</code>],
            [<code>time</code>, 'RFC 3339 partial-time', 'string'],
            [<code>uuid</code>, 'RFC 9562 UUID', 'lowercase string'],
            [<code>bytes</code>, 'binary', 'base64 string (padded)'],
            [<code>json</code>, 'any JSON value', 'as is'],
          ]}
        />
        <P>
          Composite types: <code>T[]</code> is a list (<code>T[][]</code> a list of lists),{' '}
          <code>Map&lt;string, T&gt;</code> is a map (the key must be <code>string</code>), and{' '}
          <code>Page&lt;T&gt;</code> is built in. <code>name?: T</code> marks a field that may be absent.{' '}
          <code>null</code> is not accepted for an optional field, only where the type is <code>json</code>.
          A default (<code>= literal</code>) is applied when the field is absent, so a field with a default
          is required in the output and optional in the input.
        </P>
      </Section>

      <Section id="lang-models" title="Models">
        <P>
          A <code>model</code> is a record of named fields. <code>extends</code> takes exactly one parent
          model and inherits its fields. Redeclaring an inherited field is <code>E0401</code>, and an
          inheritance cycle is <code>E0402</code>. A type may refer to itself only through <code>?</code>,{' '}
          <code>[]</code>, <code>Map</code> or a union arm, because no value of{' '}
          <code>model A {'{'} a: A {'}'}</code> could exist (<code>E0407</code>).
        </P>
        <CodeBlock title="Models" lang="veld">{`/// An amount of money. \`amount\` is a decimal string, so no precision is lost.
model Money {
  amount: decimal
  currency: Currency
}

/// Standard paging parameters for list endpoints.
model PageQuery {
  page: int = 1 @min(1)
  size: int = 20 @min(1) @max(100)
}

model Employee extends Person {
  manager?: Employee
  reports: Employee[]
}`}</CodeBlock>
        <P>
          A <code>const</code> is a named literal emitted as a constant in every target:{' '}
          <code>const MaxItems: int = 50</code>.
        </P>
      </Section>

      <Section id="lang-enums" title="Enums">
        <P>
          An <code>enum</code> is string-valued. The wire value is the variant name unless overridden with{' '}
          <code>= "WIRE_VALUE"</code>. Wire values must be unique (<code>E0403</code>). A default names a
          variant with a leading dot, as in <code>status: Status = .pending</code>.
        </P>
        <CodeBlock title="Enums" lang="veld">{`enum AccountType { checking, savings, investment }

enum Currency { USD, EUR, GBP }

enum Priority {
  low = "LOW"
  high = "HIGH"
}`}</CodeBlock>
        <P>Commas between fields, variants and arms are optional.</P>
      </Section>

      <Section id="lang-unions" title="Unions">
        <P>
          A <code>union</code> is a discriminated union whose arms are models. The discriminator property is{' '}
          <code>kind</code> unless <code>@discriminator("type")</code> says otherwise, and the arm label is
          the discriminator value. An arm model that already has a field with the discriminator's name is{' '}
          <code>E0404</code>.
        </P>
        <CodeBlock title="A union" lang="veld">{`@discriminator("type")
union Payment {
  card: Card,
  bank: BankTransfer,
}

// On the wire: { "type": "card", ...fields of Card }`}</CodeBlock>
      </Section>

      <Section id="lang-errors" title="Errors">
        <P>
          An <code>error</code> is a model with identity. It may appear only in the error position of an
          endpoint (<code>E0405</code>). The optional number is its default HTTP status and must be 4xx or
          5xx (<code>E0406</code>).
        </P>
        <CodeBlock title="Errors" lang="veld">{`/// The caller's plan does not allow this operation.
error PlanLimitReached 402 {
  limit: string
}

/// The account is frozen or closed and cannot be changed.
error AccountNotActive 409 {
  status: AccountStatus
}`}</CodeBlock>
        <P>
          Every error response is RFC 9457 <code>application/problem+json</code>, and generated clients
          throw it as a typed class. See <a href="#errors">Errors and equality</a>.
        </P>
      </Section>

      <Section id="lang-services" title="Services and endpoints">
        <P>
          <code>service Name at "/prefix"</code> groups endpoints under a path prefix that must start with{' '}
          <code>/</code>. An endpoint is{' '}
          <code>method "path" name(params) -&gt; responses</code>.
        </P>
        <CodeBlock title="examples/microservices/contracts/accounts/api.veld" lang="veld">{`/// Bank account management: open, view, freeze and close accounts.
@auth(bearer)
service Accounts at "/api/accounts" {
  /// Lists the accounts of the authenticated user.
  get "/" listAccounts() -> Account[]

  /// Fetches one account.
  get "/{id}" getAccount(id: uuid) -> Account | NotFound

  /// Opens a new account.
  post "/" createAccount(body: CreateAccountInput) -> Account

  /// Permanently closes an account.
  delete "/{id}" closeAccount(id: uuid) -> 204 | NotFound | AccountNotActive
}`}</CodeBlock>
        <Sub>Parameter binding</Sub>
        <P>Parameters are bound in this order.</P>
        <Table
          head={['Rule', 'Binding']}
          rows={[
            [<>The name appears in the path (<code>{'{id}'}</code>)</>, <>Path parameter. A scalar or enum, never optional (<code>E0502</code>).</>],
            [<>Named <code>body</code></>, <>Request body. At most one, and not on <code>get</code>, <code>head</code> or <code>options</code> (<code>E0503</code>).</>],
            [<>Named <code>query</code></>, <>A model of scalars, enums or lists of them. Its fields become query parameters (<code>E0504</code>).</>],
            [<><code>@query</code></>, 'A single query parameter, which may be optional.'],
            [<><code>@header("Header-Name")</code></>, 'A header.'],
            ['Anything else', <><code>E0505</code>, with a hint listing these options.</>],
          ]}
        />
        <Sub>Responses</Sub>
        <List>
          <li><strong>Success</strong>: at most one, status 2xx. The default is <code>201</code> for <code>post</code> and <code>200</code> otherwise. With no <code>-&gt;</code> or a bare status (<code>-&gt; 204</code>) there is no body, and the default is <code>204</code>.</li>
          <li><strong>Errors</strong>: each is an <code>error</code> declaration. The status is the one written at the use site (<code>409 OutOfStock</code>), else the error's default, else <code>E0508</code>.</li>
          <li><strong>Implicit errors</strong>: every endpoint with parameters declares <code>| ValidationError</code> (422), every endpoint with auth other than <code>none</code> declares <code>| Unauthorized</code> (401), and every endpoint declares <code>| Internal</code> (500), unless you listed them. Every response the server can produce has a declared signature.</li>
          <li>Endpoint names are unique per service (<code>E0509</code>). Method plus path is unique per package, with parameter names erased (<code>E0510</code>).</li>
        </List>
      </Section>

      <Section id="lang-streaming" title="Streaming">
        <P>
          Streaming is <strong>new in 2.0</strong>.<NewBadge /> On <code>get</code>, <code>-&gt; stream T</code>{' '}
          is Server-Sent Events. On <code>ws</code>, <code>-&gt; stream T</code> sends server-to-client
          messages and <code>body: stream T</code> receives client-to-server messages. <code>stream</code> is
          not allowed anywhere else (<code>E0511</code>).
        </P>
        <CodeBlock title="Streaming endpoints" lang="veld">{`service Live at "/live" {
  /// Server-Sent Events: one Tick per event.
  get "/ticks" ticks() -> stream Tick

  /// WebSocket: the client sends ChatIn, the server sends ChatOut.
  ws "/chat" chat(body: stream ChatIn) -> stream ChatOut
}`}</CodeBlock>
        <P>
          Streaming goes through the same pipeline as every other endpoint: routing, auth, middleware,
          validation and declared-error mapping. See <a href="#using-streaming">Using generated code</a> for
          the generated API and the wire format.
        </P>
      </Section>

      <Section id="lang-imports" title="Imports and packages">
        <P>
          Every name that does not come from the current module or the prelude is imported explicitly with{' '}
          <code>use</code>. There are no glob imports. <code>as</code> renames a name locally, and an
          import that collides with a local name is <code>E0202</code>.
        </P>
        <CodeBlock title="Three kinds of import" lang="veld">{`use users.{User, Role}                  // same package
use iam:users.{User as IamUser}          // another workspace service or library
use @acme/billing:invoices.{Invoice}     // registry package (must be in veld.lock)`}</CodeBlock>
        <List>
          <li>Imports may form cycles, between modules and between packages. Resolution collects every declaration first and resolves references second, so a cycle is never an error.</li>
          <li>Unknown package <code>E0203</code>, unknown module <code>E0204</code>, unknown name <code>E0205</code>. An unused import is the warning <code>W0201</code>.</li>
          <li>A package is a workspace service (<code>[services.x]</code>), a workspace library (<code>[libraries.x]</code>) or a registry package (<code>@org/name</code>).</li>
        </List>
      </Section>

      <Section id="lang-annotations" title="Annotations and constraints">
        <P>
          An annotation goes before a field or after it (the formatter prints it after). An unknown one is{' '}
          <code>E0601</code>; wrong arguments or target is <code>E0602</code>. Constraints are enforced by
          the generated validators on both the server and the client.
        </P>
        <Table
          head={['Annotation', 'Target', 'Meaning']}
          rows={[
            [<code>@auth(bearer | basic | none | apiKey(header: "X"))</code>, 'service, endpoint', "Auth requirement. The endpoint's value overrides the service's."],
            [<code>@middleware(Name, ...)</code>, 'service, endpoint', 'Named hooks that the generated server asks you to implement.'],
            [<code>@deprecated("msg")</code>, 'any declaration, field, variant, endpoint', 'Deprecation notice.'],
            [<><code>@min(n)</code> / <code>@max(n)</code></>, 'numeric field or param', 'Inclusive bounds.'],
            [<code>@length(min: n, max: n)</code>, 'string, list, map', 'Length bounds.'],
            [<code>@pattern("regex")</code>, 'string', 'A portable regex subset (below).'],
            [<code>@format(email | uri | hostname | ipv4 | ipv6)</code>, 'string', 'Format check.'],
            [<code>@readonly</code>, 'field', 'Set by the server. Ignored in a request body, always present in responses.'],
            [<code>@wire("name")</code>, 'field', 'Overrides the JSON property name.'],
            [<code>@example(literal)</code>, 'field, param', 'Documentation and mock data.'],
            [<code>@discriminator("prop")</code>, 'union', 'Discriminator property.'],
            [<><code>@query</code>, <code>@header("Name")</code></>, 'param', 'Binding.'],
          ]}
        />
        <P>
          <code>@pattern</code> allows literal characters, <code>.</code>, classes <code>[...]</code>,{' '}
          <code>\d \w \s</code>, the quantifiers <code>* + ? {'{n,m}'}</code>, groups, alternation and the
          anchors <code>^ $</code>. Lookaround and backreferences are <code>E0603</code>, because every
          target must enforce the same pattern.
        </P>
      </Section>

      <Section id="lang-prelude" title="Prelude errors">
        <P>These names are in scope in every module. A local declaration may shadow one (warning <code>W0202</code>).</P>
        <CodeBlock title="The prelude" lang="veld">{`error BadRequest 400      { message: string }
error Unauthorized 401    { message?: string }
error Forbidden 403       { message?: string }
error NotFound 404        { resource?: string }
error Conflict 409        { message: string }
error ValidationError 422 { issues: ValidationIssue[] }
error TooManyRequests 429 { retryAfter?: int }
error Internal 500        { message?: string }
model ValidationIssue     { path: string, code: string, message: string }`}</CodeBlock>
        <Info>
          Prelude errors belong to the reserved package <code>veld</code>, but their runtime identity still
          includes the service and operation that raised them. A <code>NotFound</code> from{' '}
          <code>orders.getOrder</code> never equals a <code>NotFound</code> from{' '}
          <code>payments.getPayment</code>.
        </Info>
      </Section>

      <Section id="lang-generics" title="Generics">
        <P>
          The only generic types are built in: <code>Page&lt;T&gt;</code> and <code>Map&lt;string, T&gt;</code>.{' '}
          <code>Page&lt;T&gt;</code> is <code>{'{ items: T[], total: long, page: int, size: int }'}</code>. User
          declarations cannot be generic: <code>Foo&lt;T&gt;</code> on one is <code>E0302</code>.
        </P>
        <CodeBlock title="Page and Map" lang="veld">{`get "/" listOrders(query: PageQuery) -> Page<Order>

model Labels {
  byLocale: Map<string, string>
}`}</CodeBlock>
      </Section>
    </>
  );
}
