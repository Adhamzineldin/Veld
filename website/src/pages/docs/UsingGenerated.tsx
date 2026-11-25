import { CodeBlock, Section, Sub, P, Info, Warn, List, Table, NewBadge } from './shared';

export default function UsingGenerated() {
  return (
    <>
      <Section id="using" title="Using generated code">
        <P>
          Veld has three built-in emitters. You write the business logic against the generated types, and
          the generated runtime does routing, auth, validation and error mapping.
        </P>
        <Table
          head={['Emitter', 'Role', 'Output']}
          rows={[
            [<code>node</code>, 'server', <>TypeScript: handler, service interface, adapters for <code>express</code>, <code>fastify</code>, <code>hono</code> and <code>node:http</code>, and a <code>nest</code> module</>],
            [<code>typescript</code>, 'client', <>Typed client classes, models, errors and validators. Add <code>framework = "react"</code> or <code>"vue"</code> for hooks or composables</>],
            [<code>java</code>, 'server and client', <>Java 17 records, sealed interfaces, a <code>java.net.http</code> client, and a JDK HTTP server or Spring Boot 3 adapter</>],
            [<code>plugin:&lt;name&gt;</code>, 'any', 'Whatever an external emitter plugin produces'],
          ]}
        />
        <P>
          If a target cannot represent a feature, generation fails with <code>E0801</code>. It never emits
          wrong code. The examples below are the real hand-written code of{' '}
          <code>examples/hello</code> and <code>examples/microservices</code>.
        </P>
      </Section>

      <Section id="using-server" title="The server">
        <P>
          Implement the generated service interface and mount its handler. Throw a generated error to answer
          with its HTTP status. Anything you throw that is not a declared error becomes a{' '}
          <code>500 Internal</code> without a message, and the original goes to your <code>onError</code>{' '}
          hook.
        </P>
        <CodeBlock title="examples/hello/server/src/main.ts" lang="ts">{`// The hello server: implement the generated \`HelloService\` and mount its handler on Express.
// \`@veld/api\` is the generated package (see \`veld setup\`), not a relative path.
import express from 'express';
import { NotFound, createHelloHandler, toExpress } from '@veld/api';
import type { GreetRequest, HelloService } from '@veld/api';

/** The business logic. Throw a generated error to answer with its HTTP status. */
export const hello: HelloService = {
  async greet({ name }: GreetRequest) {
    if (name.toLowerCase() === 'nobody') {
      throw new NotFound({ resource: name });
    }
    return { message: \`Hello, \${name}!\` };
  },
};

const app = express();
app.use(toExpress(createHelloHandler(hello)));

const port = Number(process.env.PORT ?? 3000);
app.listen(port, '127.0.0.1', () => {
  console.log(\`hello server listening on http://127.0.0.1:\${port}\`);
});`}</CodeBlock>
        <P>This is the generated interface it implements.</P>
        <CodeBlock title="examples/hello/server/src/generated/server/hello.ts (excerpt)" lang="ts">{`/** Says hello. */
export interface HelloService<Ctx = veld.RequestContext> {
  /** Greets the person with the given name. */
  greet(req: GreetRequest, ctx: Ctx): Promise<Greeting>;
}`}</CodeBlock>
        <P>
          Every request goes through one pipeline: match the route, run auth (<code>401 Unauthorized</code>),
          run middleware in declared order, bind and validate path, query, headers and body (
          <code>422 ValidationError</code> with <code>issues</code>), call your implementation, optionally
          validate the response, and serialise with the declared success status. A service that needs auth or
          middleware gets required hooks in the handler's second argument, so a missing hook is a type error.
        </P>
        <CodeBlock title="Hooks (from the cards service)" lang="ts">{`createCardsHandler(impl, { auth: { bearer: authenticate } })

// OrdersHooks: auth, middleware, onError, validateResponses
interface OrdersHooks {
  auth?: { bearer?: (token: string, ctx: RequestContext) => Promise<unknown | null> };
  middleware?: { RateLimit: (ctx: RequestContext) => Promise<void> };
  onError?: (err: unknown, ctx: RequestContext) => void;
  validateResponses?: boolean; // default false; true turns an invalid response into a 500
}`}</CodeBlock>
        <Sub id="using-rules">Handlers with rules</Sub>
        <P>
          When a service has <a href="#rules-endpoint">endpoint rules</a>, the generated code adds three things to what you
          implement. The facts an endpoint loads are handed to your handler next to its parameters. The state
          change of a <code>transition</code> is handed over as data. And the handler options gain the loaders, the
          output mode and the violation hook. This is the generated code for the bookings contract (
          <code>createBooking</code> has the fact <code>room</code>, <code>payBooking</code> has the fact <code>booking</code> and a
          transition).
        </P>
        <CodeBlock title="Generated server/bookings.ts (excerpt)" lang="ts">{`export interface PayBookingRequest {
  id: string;
  body: Payment;
}

/** The facts the loaders found, handed to the handler with the parameters. */
export interface PayBookingFacts {
  booking: Booking;
}

export interface BookingsService<Ctx = veld.RequestContext> {
  createBooking(req: CreateBookingRequest & CreateBookingFacts, ctx: Ctx): Promise<Booking>;
  payBooking(req: PayBookingRequest & PayBookingFacts & PayBookingTransition, ctx: Ctx): Promise<Booking>;
}

/** One loader per fact of \`Bookings\`. A loader resolves to \`null\` or \`undefined\` when it finds nothing. */
export interface BookingsFactLoaders {
  booking(key: string, ctx: veld.RequestContext): Promise<Booking | null | undefined>;
  room(key: string, ctx: veld.RequestContext): Promise<Room | null | undefined>;
}

export interface BookingsHooks {
  facts: BookingsFactLoaders;
  clock?: (ctx: veld.RequestContext) => Date;
  auth: { bearer: (token: string, ctx: veld.RequestContext) => Promise<User | null> };
  onError?: (err: unknown, ctx: veld.RequestContext) => void;
  validateResponses?: boolean;
  rules?: { output?: veld.RuleOutputMode };               // 'strict' (default), 'audit' or 'off'
  onRuleViolation?: (event: veld.RuleViolationEvent) => void;
}`}</CodeBlock>
        <List>
          <li><strong>Facts.</strong> <code>facts</code> is required by the type system when the service has facts. The handler gets the loaded value, so it does not load it again. A fact that is not found is a <code>404</code> before the handler runs.</li>
          <li><strong>The principal.</strong> The auth hook returns the generated type of the contract's <code>principal</code> (<code>User</code> here), or <code>null</code> for a <code>401</code>. A handler reads it as <code>ctx.principal</code>, typed <code>unknown</code>, so narrow it once: <code>ctx.principal as User</code>.</li>
          <li><strong>The transition.</strong> <code>transition</code> is <code>{"{ field, from, to }"}</code>, typed with the enum. Write the change only while the row still holds <code>from</code>, and throw <code>InvalidTransition</code> if it does not. See <a href="#rules-states">State machines</a>.</li>
          <li><strong>The clock.</strong> <code>now</code> in a rule is read once per request from <code>clock</code>. The default is the system clock.</li>
          <li><strong>The hooks.</strong> <code>rules.output</code> and <code>onRuleViolation</code> are described under <a href="#rules-output">Output mode and hook</a>.</li>
        </List>
        <P>
          With <code>testing = true</code> on the target (<a href="#config">veld.toml</a>), a second file{' '}
          <code>server/&lt;service&gt;-test-handler.ts</code> exports <code>create&lt;Service&gt;TestHandler</code>, which
          <code>veld test --live</code> talks to. It is never exported from <code>index.ts</code> and must never be mounted in
          production. See <a href="#rules-testing">Examples and veld test</a>.
        </P>
      </Section>

      <Section id="using-adapters" title="Server adapters">
        <P>
          An adapter connects the handler to a framework. The TypeScript adapters use structural types, so
          they import nothing and the generated server has no dependencies. Choose one with{' '}
          <code>framework</code> on the target.
        </P>
        <Table
          head={['framework', 'Emitter', 'Mount it with']}
          rows={[
            [<code>express</code>, 'node', <code>app.use(toExpress(handler))</code>],
            [<code>fastify</code>, 'node', <code>app.all('/*', toFastify(handler))</code>],
            [<code>hono</code>, 'node', <code>app.use('*', toHono(handler))</code>],
            [<code>node-http</code>, 'node', <code>http.createServer(toNodeHttp(handler))</code>],
            [<><code>nest</code> (<code>"10"</code>)</>, 'node', <>A generated <code>@Controller</code> and a <code>&lt;Service&gt;VeldModule</code></>],
            [<code>jdk</code>, 'java', <code>JdkHttpServerAdapter.listen(handler, address)</code>],
            [<><code>spring</code> (<code>"3"</code>)</>, 'java', <>A generated <code>@RestController</code> per service</>],
          ]}
        />
        <CodeBlock title="Fastify: examples/microservices/backend/card-service" lang="ts">{`import Fastify from 'fastify';
import { createCardsHandler, toFastify } from '@veld/cards-api';

const app = Fastify();
app.all('/*', toFastify(createCardsHandler(impl, { auth: { bearer: authenticate } })));
app.listen({ port, host: process.env.HOST ?? '127.0.0.1' });`}</CodeBlock>
        <CodeBlock title="Hono: examples/microservices/backend/notification-service" lang="ts">{`import { serve } from '@hono/node-server';
import { Hono } from 'hono';
import { createNotificationsHandler, toHono } from '@veld/notifications-api';

const app = new Hono();
app.use('*', toHono(createNotificationsHandler(impl, { auth: { bearer: authenticate } })));
serve({ fetch: app.fetch, port, hostname: process.env.HOST ?? '127.0.0.1' });`}</CodeBlock>
        <Sub>NestJS</Sub>
        <P>
          With <code>framework = "nest"</code> Veld adds <code>server/nest/&lt;service&gt;.ts</code>: a controller
          with one decorated method per endpoint at the real paths, so Nest and Swagger see the real routes.
          Each method delegates to the same handler as every other adapter, so status codes and bodies are
          exact. The tokens <code>ORDERS_SERVICE</code> and <code>ORDERS_HOOKS</code> are the DI tokens.
        </P>
        <CodeBlock title="Nest module" lang="ts">{`@Module({ imports: [OrdersVeldModule.forRoot({ useClass: OrdersImpl, hooks })] })
class AppModule {}`}</CodeBlock>
        <Warn>
          Nest needs <code>experimentalDecorators</code> and <code>emitDecoratorMetadata</code> in{' '}
          <code>tsconfig.json</code>, but only for <code>server/nest/**</code>. A global prefix or URI
          versioning is not supported: mount the module under the contract's own prefix.
        </Warn>
        <Sub>JDK and Spring</Sub>
        <P>
          The Java server is described in the Java section below. <code>jdk</code> uses{' '}
          <code>com.sun.net.httpserver</code> and needs nothing else. <code>spring</code> (Spring Boot 3,
          Jakarta, Java 17 or newer) is native: one <code>@RestController</code> per service with a mapping
          per endpoint at its real path, so actuator and springdoc see the real routes. Register the
          controllers by component-scanning <code>&lt;base&gt;.adapters</code>.
        </P>
      </Section>

      <Section id="using-client" title="The TypeScript client">
        <P>
          The <code>typescript</code> emitter generates one client class per service. Base URLs come from the
          constructor, an environment variable or <code>veld.toml</code> (see{' '}
          <a href="#ms-base-urls">Base URLs</a>). The client validates the request before sending it, and
          throws typed errors.
        </P>
        <CodeBlock title="examples/library-package/web/src/report.ts" lang="ts">{`// \`@veld/web\` is the generated client package (see \`veld setup\`). The errors from the
// \`common\` library are imported from it like every other generated type.
import { InvoicesClient, OrdersClient, PlanLimitReached, Unavailable } from '@veld/web';

const auth = { bearer: () => process.env.API_TOKEN ?? 'demo-token' };
// Base URLs come from VELD_ORDERS_URL and VELD_INVOICES_URL.
const orders = new OrdersClient({ auth });
const invoices = new InvoicesClient({ auth });

try {
  const order = await orders.createOrder({ body: { total: { amount: '49.90', currency: 'EUR' }, note: 'report' } });
  console.log(order.id, order.status);
} catch (e) {
  if (!PlanLimitReached.matches(e, 'orders/Orders')) throw e;
  console.log(\`plan limit reached: \${e.data.limit}\`);
}`}</CodeBlock>
        <List>
          <li>A request type per endpoint holds path parameters by name, <code>body</code>, <code>query</code>, single <code>@query</code> parameters and headers.</li>
          <li><code>ClientOptions</code>: <code>baseUrl</code>, <code>fetch</code>, <code>headers</code> (object or function) and <code>auth</code> (<code>bearer</code>, <code>basic</code>, <code>apiKey</code>).</li>
          <li>Pass <code>{'{ signal }'}</code> as the second argument to cancel a call.</li>
          <li>A problem+json <code>type</code> that matches a declared error is thrown as that class. Any other non-2xx response is a <code>VeldHttpError</code> with <code>status</code> and <code>body</code>. A network failure is a <code>VeldTransportError</code>.</li>
        </List>
      </Section>

      <Section id="using-react" title="React hooks">
        <P>
          Set <code>framework = "react"</code> on the client target (React 18). The hooks are thin wrappers
          over the generated clients, use only React's own API, and keep the typed errors. Endpoint hooks are
          named <code>use&lt;Endpoint&gt;</code>. A <code>get</code> endpoint is a query, every other method is
          a mutation.
        </P>
        <CodeBlock title="examples/hello/web/src/App.tsx" lang="ts">{`import { HelloClient } from '@veld/web';
import { VeldProvider } from '@veld/web/hello/react';
import { Greeter } from './Greeter';

/** One client for the whole app. Same origin: Vite proxies \`/hello\` to the server in development. */
const client = new HelloClient({ baseUrl: window.location.origin });

export function App() {
  return (
    <VeldProvider clients={{ hello: client }}>
      <h1>Hello</h1>
      <Greeter />
    </VeldProvider>
  );
}`}</CodeBlock>
        <CodeBlock title="examples/hello/web/src/Greeter.tsx" lang="ts">{`import { useState } from 'react';
import { NotFound } from '@veld/web';
import { useGreet } from '@veld/web/hello/react';

export function Greeter() {
  const [name, setName] = useState('');
  // A null request holds the call back until there is a name.
  const greeting = useGreet(name === '' ? null : { name });
  return (
    <section>
      <label>
        Name <input value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      {greeting.loading && <p role="status">Loading...</p>}
      {greeting.data && <p role="status">{greeting.data.message}</p>}
      {NotFound.matches(greeting.error) && <p role="alert">Nobody called {name} was found.</p>}
    </section>
  );
}`}</CodeBlock>
        <List>
          <li><strong>Queries</strong>: <code>useGetOrder(req | null, opts?)</code> returns <code>{'{ data, error, loading, refetch }'}</code>. A <code>null</code> request or <code>enabled: false</code> holds the call back. The call re-runs when the canonical bytes of the request change, so a new but equal object does not refetch.</li>
          <li><strong>Mutations</strong>: <code>useCreateOrder()</code> returns <code>{'{ mutate, data, error, loading, reset }'}</code>. <code>mutate</code> never rejects: a failure lands in <code>error</code>.</li>
          <li><code>&lt;VeldProvider clients=&#123;&#123; orders: client &#125;&#125;&gt;</code> provides the clients, and <code>useOrdersClient()</code> reads them.</li>
        </List>
      </Section>

      <Section id="using-vue" title="Vue composables">
        <P>
          Set <code>framework = "vue"</code> (Vue 3.3 or newer). The names are the same as for React, but{' '}
          <code>data</code>, <code>error</code> and <code>loading</code> are refs, and the request may be a
          ref or a getter, so <code>useGetOrder(reqRef)</code> reacts to changes. The composables use{' '}
          <code>watch</code> and <code>onScopeDispose</code>, so they work in components and in a bare{' '}
          <code>effectScope</code>. <code>createVeldPlugin(&#123; orders: client &#125;)</code> provides the
          clients with <code>app.use(...)</code>.
        </P>
        <Info>
          The framework files are the only generated files that import a package, and only the one you
          chose. They are not in the barrel, so importing the barrel never needs React or Vue installed:
          import <code>&lt;package&gt;/react</code> or <code>&lt;package&gt;/vue</code> explicitly.
        </Info>
      </Section>

      <Section id="using-java" title="Java server and client">
        <P>
          The <code>java</code> emitter produces Java 17 code that compiles with{' '}
          <code>javac --release 17</code> and <code>--release 21</code> with no warnings. Models are records,
          enums are Java enums, unions are sealed interfaces, and errors are unchecked exceptions. Generated
          code imports only the JDK; the optional Spring adapter imports Spring Boot 3. Set the base package
          with the target option <code>package</code>.
        </P>
        <CodeBlock title="examples/microservices/backend/transaction-service (TransactionServiceMain.java)" lang="java">{`VeldHandler handler =
    TransactionsHandler.create(
        new TransactionsServiceImpl(), new TransactionsHooks(TransactionsServiceImpl::authenticate));
JdkHttpServerAdapter.listen(handler, new InetSocketAddress(host, port));`}</CodeBlock>
        <P>
          <code>TransactionsServiceImpl</code> implements the generated <code>TransactionsService</code>.
          A typed error from another service becomes this service's own error:
        </P>
        <CodeBlock title="TransactionsServiceImpl.java (excerpt)" lang="java">{`AccountsClient accounts = new AccountsClient(ClientOptions.builder().bearer(p::token).build());
try {
  // ...
} catch (com.nexusbank.transactions.clients.accounts.prelude.NotFound e) {
  // The accounts service's typed NotFound becomes this service's own NotFound.
  throw new NotFound(new NotFound.Data("account"));
}`}</CodeBlock>
        <P>The Java client is blocking and built on <code>java.net.http.HttpClient</code>.</P>
        <CodeBlock title="A Java client" lang="java">{`OrdersClient client = new OrdersClient(ClientOptions.builder()
    .baseUrl("https://api.example.com").bearer(() -> token).build());
Order order = client.getOrder(new GetOrderRequest(id));`}</CodeBlock>
        <List>
          <li><code>ClientOptions</code> has <code>baseUrl</code>, <code>httpClient</code>, <code>headers</code>, <code>bearer</code>, <code>basic</code>, <code>apiKey</code>, <code>validate</code> (default true) and <code>timeout</code> (default 30 s).</li>
          <li>Optional fields and absent optional parameters are <code>null</code>, and <code>long</code> is a decimal string on the wire.</li>
          <li><code>catch (OutOfStock e)</code> is reliable only inside one generated package. Across packages use <code>OutOfStock.matches(e)</code> or <code>Veld.same(a, b)</code> (see <a href="#equality">same() and matches()</a>).</li>
          <li><code>veld setup</code> adds the generated sources to your Maven or Gradle build, so you import the generated packages normally.</li>
        </List>
      </Section>

      <Section id="using-streaming" title="Streaming">
        <P>
          Streaming <NewBadge /> uses the same pipeline as every other endpoint. The Node server, the
          TypeScript client (with React hooks and Vue composables) and Java support it. A Java server with{' '}
          <code>framework = "jdk"</code> reports <code>E0801</code> for <code>ws</code> endpoints, because the
          JDK has no WebSocket server: use <code>framework = "spring"</code>.
        </P>
        <Sub>Server-Sent Events</Sub>
        <P>
          <code>get ... -&gt; stream T</code> answers <code>200 text/event-stream</code>. The implementation
          returns an <code>AsyncIterable</code> (an <code>async *</code> generator is enough), and may yield{' '}
          <code>veld.sseEvent(value, {'{ event, id, retry }'})</code> to set the SSE fields. The first item is
          pulled before the status is sent, so an error raised before the first event is a normal
          problem+json response. After streaming has started, an error is sent as a final{' '}
          <code>event: error</code> and the stream closes.
        </P>
        <CodeBlock title="Client: an AsyncIterable" lang="ts">{`for await (const tick of live.ticks({}, { signal })) {
  console.log(tick);
}
// break or abort ends the iteration and closes the connection.
// There is no automatic reconnect: call again with lastEventId to resume.`}</CodeBlock>
        <Sub>WebSocket</Sub>
        <P>
          The server implements <code>chat(req, ctx, ws)</code>: <code>ws.incoming</code> is a validated{' '}
          <code>AsyncIterable</code> of client messages and <code>ws.send(out)</code> sends one. Returning ends
          the socket. Mount it with <code>attachWebSocket(new WebSocketServer(&#123; server &#125;), handler)</code>.
          The client's <code>chat(req, &#123; signal &#125;)</code> returns a channel you iterate and{' '}
          <code>send</code> to. Browsers cannot set headers on a WebSocket, so bearer and API-key auth also
          accept <code>?access_token=</code> and <code>?api_key=</code>. A token in a URL can end up in logs:
          prefer short-lived tokens.
        </P>
        <Sub>Java</Sub>
        <P>
          The wire behaviour is the TypeScript target's, byte for byte. A Java server method for an SSE
          endpoint returns a blocking <code>Stream&lt;SseEvent&lt;T&gt;&gt;</code> (the server closes it when the
          client disconnects, so use <code>Stream.onClose</code> to clean up), and a <code>ws</code> endpoint
          receives a <code>WsSession</code>.
        </P>
        <CodeBlock title="Java server methods" lang="java">{`Stream<SseEvent<Tick>> ticks(TicksRequest request, RequestContext ctx) throws Exception;     // get ... -> stream Tick
void chat(ChatRequest request, RequestContext ctx, WsSession<Say, Heard> ws) throws Exception; // ws ... body: stream Say -> stream Heard`}</CodeBlock>
        <P>
          In React and Vue, each streaming endpoint gets a hook,{' '}
          <code>use&lt;Endpoint&gt;Stream(req | null, opts?)</code>, returning{' '}
          <code>{'{ items, latest, error, streaming, restart }'}</code> (and <code>send</code> for a{' '}
          <code>ws</code> endpoint). The stream re-opens when the request changes and closes on unmount.
        </P>
        <Warn>
          WebSocket is the one exception to zero dependencies: a Node server that hosts <code>ws</code>{' '}
          endpoints lists the <code>ws</code> package as a peer dependency (the adapter itself imports
          nothing), and a Spring server needs <code>spring-boot-starter-websocket</code>. Clients use the
          platform <code>WebSocket</code> and <code>java.net.http.WebSocket</code>.
        </Warn>
      </Section>
    </>
  );
}
