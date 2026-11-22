import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';

const c = (s: string) => <code>{s}</code>;

export default function Configuration() {
  return (
    <Section id="config" title="Configuration reference: veld.toml">
      <P>
        Each project has exactly one config file, <code>veld.toml</code>, at the project root. Veld finds it by
        searching upward from the current directory (or use <code>--config &lt;PATH&gt;</code>). Paths in it are
        relative to the file. <strong>An unknown key is an error</strong> (<code>E0701</code>), and the message
        lists the closest valid key.
      </P>

      <Sub>Single service</Sub>
      <CodeBlock title="veld.toml" lang="toml">{`[project]
name = "shop"              # package name of the implicit default service
contracts = "contracts"    # package root (default "contracts")

[[targets]]
role = "server"
emitter = "node"
framework = "express"      # express | fastify | hono | node-http | nest
out = "../api/src/generated"

[[targets]]
role = "client"
emitter = "typescript"
out = "../web/src/generated"`}</CodeBlock>

      <Sub>Workspace</Sub>
      <CodeBlock title="veld.toml" lang="toml">{`[project]
name = "nexusbank"

[libraries.common]          # shared contracts: use common:money.{Money}
contracts = "contracts/common"

[services.orders]
contracts = "contracts/orders"
base-url = "http://orders:3002"
consumes = ["iam", "payments"]   # cycles allowed

[[targets]]
name = "orders-api"
service = "orders"
role = "server"
emitter = "node"
framework = "express"
out = "services/orders/src/generated"

[plugins.mylang]
command = "veld-emit-mylang"
version = "1.2.0"
sha256 = "..."

[registry]
url = "https://registry.example.com"

[dependencies]
"@acme/billing" = "^1.2"`}</CodeBlock>

      <Sub>Every key</Sub>
      <Table
        head={['Section and key', 'Meaning']}
        rows={[
          [c('[project] name'), 'Required. The project name, and the package name of the implicit service when there is no [services] section.'],
          [c('[project] contracts'), 'Package root (default "contracts"). Exactly one of this or [services.*] defines the packages.'],
          [c('[project] package, version, description'), <>Registry metadata for <code>veld publish</code>: <code>package = "@org/name"</code>, a semver <code>version</code>, an optional <code>description</code>. Also valid on <code>[libraries.*]</code> and <code>[services.*]</code>.</>],
          [c('[libraries.<name>] contracts'), 'Directory of a shared package. A library has no endpoints and no target.'],
          [c('[services.<name>] contracts'), 'Directory of the service package.'],
          [c('[services.<name>] base-url'), 'Default base URL of the service for generated clients.'],
          [c('[services.<name>] consumes'), <>Services this one calls. Must exist (<code>E0702</code>). Cycles are allowed. Each gets a generated client in server targets.</>],
          [c('[[targets]] name'), <>Optional name, used by <code>veld generate &lt;name&gt;</code>, <code>veld setup &lt;name&gt;</code> and as the package name <code>@veld/&lt;name&gt;</code>.</>],
          [c('[[targets]] role'), <>Required. <code>"server"</code> or <code>"client"</code>.</>],
          [c('[[targets]] emitter'), <>Required. <code>"node"</code>, <code>"typescript"</code>, <code>"java"</code> or <code>"plugin:&lt;name&gt;"</code>.</>],
          [c('[[targets]] out'), <>Required. Output directory. Two targets must not share one (<code>E0703</code>), and it must not be the project root or a contracts directory (<code>E0704</code>).</>],
          [c('[[targets]] framework'), <>Optional framework, validated by the emitter for the role. Unknown is <code>E0803</code>, and the message lists the valid values.</>],
          [c('[[targets]] service'), <>Required for <code>role = "server"</code>: the service it serves.</>],
          [c('[[targets]] services'), <>Client targets: the services to cover. Default: all.</>],
          [c('[[targets]] clients'), <>Server targets: <code>false</code> turns off the generated clients of consumed services.</>],
          [c('[[targets]] app'), <>The app that consumes the output, for <code>veld setup</code> (relative to <code>veld.toml</code>).</>],
          [c('[targets.options]'), 'Emitter-specific options, validated against the emitter (unknown key: E0802). See the next table.'],
          [c('[plugins.<name>] command'), <>The plugin executable, resolved on <code>PATH</code> or as a relative path.</>],
          [c('[plugins.<name>] version'), 'The plugin version.'],
          [c('[plugins.<name>] entry'), <>Optional script that an interpreter <code>command</code> runs. Veld appends it to <code>command</code> unless already present.</>],
          [c('[plugins.<name>] sha256'), <>Hash of <code>entry</code> if set, else of the resolved executable. A mismatch is refused (<code>E0705</code>). For a script plugin always set <code>entry</code>.</>],
          [c('[registry] url'), <>The registry. <code>--registry</code> and <code>VELD_REGISTRY</code> override it. Must be https, except localhost.</>],
          [c('[dependencies]'), <>Registry packages: <code>"@org/name" = "^1.2"</code>, resolved and pinned in <code>veld.lock</code>.</>],
        ]}
      />

      <Sub>Frameworks and options per emitter</Sub>
      <Table
        head={['Emitter', 'Role', 'framework values (versions)', 'Options']}
        rows={[
          [c('node'), 'server', <>{c('express')}, {c('fastify')}, {c('hono')}, {c('node-http')} (unversioned), {c('nest')} ({c('"10"')})</>, <><code>typescript</code>, <code>framework-version</code>, <code>package-name</code></>],
          [c('typescript'), 'client', <>none (plain client), {c('react')} ({c('"18"')}), {c('vue')} ({c('"3"')})</>, <><code>typescript</code>, <code>framework-version</code>, <code>package-name</code></>],
          [c('java'), 'server', <>{c('jdk')} (unversioned), {c('spring')} ({c('"3"')})</>, <><code>package</code>, <code>java</code>, <code>framework-version</code></>],
          [c('java'), 'client', 'none', <><code>package</code>, <code>java</code></>],
        ]}
      />
      <Table
        head={['Option', 'Emitters', 'Values', 'Default']}
        rows={[
          [c('java'), 'java', <><code>"17"</code>, <code>"21"</code></>, <code>"17"</code>],
          [c('typescript'), 'typescript, node', <><code>"es2020"</code>, <code>"es2022"</code></>, <code>"es2020"</code>],
          [c('framework-version'), 'any emitter whose framework has versions', 'the framework\'s versions', 'the first version'],
          [c('package'), 'java', 'a valid Java package name', <code>dev.veld.generated.&lt;package&gt;</code>],
          [c('package-name'), 'node, typescript', <>the npm package name</>, <code>@veld/&lt;target name&gt;</code>],
          [c('validate'), 'node, typescript, java', <><code>true</code>, <code>false</code> (skip run-time request validation, keep the types)</>, <code>true</code>],
          [c('query-options'), 'typescript', <><code>true</code> writes <code>queries.ts</code>: query keys and query options as plain objects, no dependency</>, <code>false</code>],
        ]}
      />
      <List>
        <li>An unknown language version is <code>E0802</code>. A <code>framework-version</code> outside the adapter's list, one without a <code>framework</code>, or one for an unversioned framework is <code>E0803</code>. A language older than the framework needs is <code>E0803</code>. All are reported before generating, with a message that lists the valid values.</li>
        <li>The list of frameworks comes from each emitter's adapter registry, so this table, the <code>E0803</code> message and the generated adapters cannot drift apart.</li>
      </List>
      <CodeBlock title="A Spring server on Java 21" lang="toml">{`[[targets]]
role = "server"
service = "orders"
emitter = "java"
framework = "spring"
out = "services/orders/src/main/java"

[targets.options]
java = "21"
framework-version = "3"
package = "com.acme.orders"`}</CodeBlock>

      <Sub>Rules</Sub>
      <List>
        <li>With no <code>[services]</code> section the project itself is one service named <code>project.name</code>.</li>
        <li>Generated output never contains absolute paths, user names, host names or timestamps.</li>
        <li>The base URL of a client is resolved from the constructor, then <code>VELD_&lt;SERVICE&gt;_URL</code>, then <code>base-url</code>.</li>
      </List>

      <Sub>Generated files next to veld.toml</Sub>
      <Table
        head={['File', 'What']}
        rows={[
          [c('veld.lock.ir.json'), <>Written by a full <code>veld generate</code>: the compiled IR as pretty, sorted JSON. A snapshot of the contract as last generated. <strong>Commit it.</strong> <code>veld diff</code> compares against it.</>],
          [c('veld.lock'), <>TOML. Pinned registry packages with checksums (see <a href="#registry">Registry</a>). Distinct from <code>veld.lock.ir.json</code>.</>],
          [c('<out>/.veld-manifest.json'), 'The files Veld wrote into an output directory, with their content hashes.'],
          [c('.veld/packages/'), 'Installed registry packages (gitignored).'],
        ]}
      />
      <Info>
        Credentials are stored per registry URL in <code>~/.veld/credentials.toml</code> (
        <code>$VELD_HOME</code> overrides the directory) and are never printed.
      </Info>
    </Section>
  );
}
