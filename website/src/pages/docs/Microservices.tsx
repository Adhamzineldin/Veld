import { CodeBlock, Section, P, Info, List, Table } from './shared';

export default function Microservices() {
  return (
    <>
      <Section id="microservices" title="Microservices: workspaces and consumes">
        <P>
          A workspace lists services, and each service says which others it calls with{' '}
          <code>consumes</code>. <strong>Cycles are legal</strong>: services may call each other, and
          modules may import each other's types across services.
        </P>
        <CodeBlock title="examples/microservices/veld.toml (excerpt)" lang="toml">{`[project]
name = "nexusbank"

[services.iam]
contracts = "contracts/iam"
base-url = "http://iam-service:3001"

# accounts and transactions consume each other. Cycles are legal.
[services.accounts]
contracts = "contracts/accounts"
base-url = "http://account-service:3002"
consumes = ["iam", "transactions"]

[services.transactions]
contracts = "contracts/transactions"
base-url = "http://transaction-service:3003"
consumes = ["iam", "accounts"]`}</CodeBlock>
        <P>
          The contracts import across services with <code>use</code>.{' '}
          <code>accounts</code> declares a <code>Statement</code> that holds{' '}
          <code>Transaction[]</code>, while <code>transactions</code> imports <code>Account</code>:
        </P>
        <CodeBlock title="contracts/accounts/accounts.veld" lang="veld">{`use transactions:transactions.{Transaction}

/// An account together with its latest transactions.
/// \`Transaction\` comes from the \`transactions\` service, which in turn imports \`Account\`.
model Statement {
  account: Account
  recent: Transaction[]
}`}</CodeBlock>
        <P>
          Targets of different languages can sit in one workspace. In the <code>microservices</code>{' '}
          example, four servers are Node (Express, Fastify, Hono) and the <code>transactions</code> service
          is Java on the JDK adapter, with a React client for the web app that covers every service. A typed
          error crosses every hop: a transfer from an unknown account on the Java service calls the Node{' '}
          <code>accounts</code> service, which raises <code>NotFound</code>. Java turns it into its own{' '}
          <code>NotFound</code>, and the React app receives the typed class.
        </P>
        <CodeBlock title="Targets (excerpt)" lang="toml">{`[[targets]]
name = "accounts-api"
service = "accounts"
role = "server"
emitter = "node"
framework = "express"
out = "backend/account-service/src/generated"

# The transactions service is a Java server (JDK HTTP server adapter, no dependencies).
[[targets]]
name = "transactions-api"
service = "transactions"
role = "server"
emitter = "java"
framework = "jdk"
out = "backend/transaction-service/src/main/java"

[targets.options]
package = "com.nexusbank.transactions"

# One TypeScript client with React hooks for the web app, covering every service.
[[targets]]
name = "web"
role = "client"
emitter = "typescript"
framework = "react"
out = "web/src/generated"`}</CodeBlock>
        <List>
          <li><code>consumes</code> entries must name existing services (<code>E0702</code>). Listing the service itself is the warning <code>W0701</code>.</li>
          <li>A server target needs <code>service</code>. A client target takes <code>services = [...]</code> and defaults to all of them.</li>
          <li><code>veld generate accounts-api</code> generates one target by name.</li>
        </List>
      </Section>

      <Section id="ms-clients" title="Service clients">
        <P>
          Every server target also gets a generated client for each consumed service, in the server's own
          language, under <code>&lt;out&gt;/clients/&lt;service&gt;/</code>. Set <code>clients = false</code> on
          the target to opt out. In TypeScript they come through the same package as namespaces:
        </P>
        <CodeBlock title="examples/microservices/backend/account-service" lang="ts">{`import { createAccountsHandler, iam, transactions } from '@veld/accounts-api';

const profile = await new iam.IamClient({ auth: { bearer: () => token } }).getProfile({});
const page = await new transactions.TransactionsClient({ auth: { bearer: () => token } })
  .getHistory({ accountId, query: { page: 1, size: 5 } });`}</CodeBlock>
        <P>
          In Java the same clients are packages under <code>&lt;base&gt;.clients.&lt;service&gt;</code>, imported
          like any other class.
        </P>
        <CodeBlock title="Java" lang="java">{`import com.nexusbank.transactions.clients.accounts.accounts.client.AccountsClient;
import com.nexusbank.transactions.transactions.server.TransactionsService;`}</CodeBlock>
      </Section>

      <Section id="ms-base-urls" title="Base URLs">
        <P>A generated client resolves its base URL in this order, highest priority first.</P>
        <Table
          head={['#', 'Source', 'Example']}
          rows={[
            ['1', 'The constructor argument', <code>new OrdersClient(&#123; baseUrl: '...' &#125;)</code>],
            ['2', <>The environment variable <code>VELD_&lt;SERVICE&gt;_URL</code> (service name in UPPER_SNAKE)</>, <code>VELD_ORDERS_URL=http://127.0.0.1:3001</code>],
            ['3', <>The <code>base-url</code> in <code>veld.toml</code></>, <code>base-url = "http://orders:3001"</code>],
            ['4', 'Otherwise', <>An error at client construction (<code>VeldTransportError</code>)</>],
          ]}
        />
        <Info>
          Without Docker, every service of the example reads <code>PORT</code> and the{' '}
          <code>VELD_&lt;SERVICE&gt;_URL</code> variables for the services it calls, so one set of contracts
          runs in Compose, in Kubernetes or on a laptop.
        </Info>
      </Section>

      <Section id="ms-libraries" title="Shared libraries">
        <P>
          Shared contracts live in a <code>[libraries.&lt;name&gt;]</code> package. A library has no endpoints
          and no target. Its declarations are generated into every target that uses them. Because a shared
          error is declared once, it has one signature everywhere.
        </P>
        <CodeBlock title="examples/library-package/veld.toml (excerpt)" lang="toml">{`[libraries.common]
contracts = "contracts/common"

[services.orders]
contracts = "contracts/orders"
base-url = "http://orders:3001"`}</CodeBlock>
        <CodeBlock title="contracts/orders/orders.veld (excerpt)" lang="veld">{`use common:errors.{PlanLimitReached, Unavailable}
use common:money.{Money, PageQuery}

service Orders at "/orders" {
  /// Places an order.
  post "/" createOrder(body: CreateOrder) -> Order | PlanLimitReached | Unavailable
}`}</CodeBlock>
        <P>
          An <code>Unavailable</code> raised by one service is recognised by code generated for the other.
          An error declared in a library is matched together with the service that raised it, so the client
          writes <code>Unavailable.matches(e, 'invoices/Invoices')</code>.
        </P>
      </Section>
    </>
  );
}
