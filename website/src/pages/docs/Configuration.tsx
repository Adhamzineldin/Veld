import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';

const c = (s: string) => <code>{s}</code>;

export default function Configuration() {
  return (
    <Section id="config" title="Configuration reference: veld.toml">
      <P>
        A project is a <strong>workspace</strong>: a root <code>veld.toml</code> that lists the members and holds what
        is shared, and one small <code>veld.toml</code> per member (a service, an app or a library) in the
        member&apos;s own directory. A project with one member keeps one file. Veld finds the file by searching upward
        from the current directory (or use <code>--config &lt;PATH&gt;</code>). Paths in a file are relative to that
        file. <strong>An unknown key is an error</strong> (<code>E0701</code>), and the message lists the closest
        valid key. A file in the layout of earlier betas (<code>[project]</code>, <code>[services.*]</code>,{' '}
        <code>[libraries.*]</code> or <code>[[targets]]</code>) is <code>E0709</code>, and the message says what to
        write instead.
      </P>

      <Sub>Single file</Sub>
      <CodeBlock title="veld.toml" lang="toml">{`[service]
name = "shop"              # package name of the service
contracts = "contracts"    # package root (default "contracts")

[server]
emitter = "node"
framework = "express"      # express | fastify | hono | node-http | nest
out = "../api/src/generated"

[client]
emitter = "typescript"
out = "../web/src/generated"`}</CodeBlock>

      <Sub>Workspace</Sub>
      <CodeBlock title="veld.toml (the root)" lang="toml">{`[workspace]
name = "nexusbank"
members = ["services/*", "apps/*", "libs/*"]

[workspace.defaults.server.node]   # a target of that role and emitter starts from these
framework = "express"

[plugins.mylang]
command = "veld-emit-mylang"
version = "1.2.0"
sha256 = "..."

[registry]
url = "https://registry.example.com"

[dependencies]
"@acme/billing" = "^1.2"`}</CodeBlock>
      <CodeBlock title="services/orders/veld.toml" lang="toml">{`[service]
consumes = ["iam", "payments"]   # cycles allowed; the name defaults to the directory name
base-url = "http://orders:3002"
owners = ["team-orders"]

[server]
name = "orders-api"
emitter = "node"
out = "src/generated"            # relative to this file`}</CodeBlock>
      <CodeBlock title="apps/web/veld.toml and libs/common/veld.toml" lang="toml">{`[app]
uses = ["orders"]                # default: every service

[client]
emitter = "typescript"
framework = "react"

# libs/common/veld.toml: shared contracts, use common:money.{Money}
[library]`}</CodeBlock>

      <Sub>Every key</Sub>
      <Table
        head={['Section and key', 'Meaning']}
        rows={[
          [c('[workspace] name'), 'Required. The project name. In the single file form the name of the member is the project name.'],
          [c('[workspace] members, exclude'), <>Member directories relative to the root, with <code>/</code>. A segment may be <code>*</code>; nothing else is a pattern. A directory without a <code>veld.toml</code> is skipped.</>],
          [c('[workspace] prefix, description'), <>A route prefix such as <code>/api</code> prepended to every service prefix; a one-line description.</>],
          [c('[workspace.defaults.<role>.<emitter>]'), <><code>framework</code> and option keys a target of that role and emitter starts from. What the member writes replaces the default key by key.</>],
          [c('[service] name'), 'The service name, which use and consumes refer to. Default: the directory name. Required in the root file.'],
          [c('[service] contracts'), 'Directory of the service package, relative to the file (default "contracts").'],
          [c('[service] base-url'), 'Default base URL of the service for generated clients.'],
          [c('[service] consumes'), <>Services this one calls. Must exist (<code>E0702</code>). Cycles are allowed. Each gets a generated client in server targets.</>],
          [c('[service] prefix, owners'), <>A route prefix such as <code>/v1</code>, added after the workspace prefix. <code>owners</code> are free-form names shown by <code>veld diff</code> and <code>veld check --graph=json</code>.</>],
          [c('[service] package, version, description'), <>Registry metadata for <code>veld publish</code>: <code>package = "@org/name"</code>, a semver <code>version</code>, an optional <code>description</code>. Also valid on <code>[library]</code>.</>],
          [c('[app] name, uses, owners'), <>An app has clients and no server. <code>uses</code> names the services its clients cover (default: all). Must exist (<code>E0702</code>).</>],
          [c('[library] name, contracts, owners'), 'A shared package. A library has no endpoints and no target (a target there is E0711).'],
          [c('[server] or [client] name'), <>Optional name, used by <code>veld generate &lt;name&gt;</code>, <code>veld setup &lt;name&gt;</code> and as the package name <code>@veld/&lt;name&gt;</code>. Default: the member&apos;s name, or <code>&lt;member&gt;-server</code> and <code>&lt;member&gt;-client</code> when it has both.</>],
          [c('[server] or [client] emitter'), <>Required. <code>"node"</code>, <code>"typescript"</code>, <code>"java"</code>, <code>"rust"</code> or <code>"plugin:&lt;name&gt;"</code>. The table name is the role; write <code>[[server]]</code> for more than one target of a role.</>],
          [c('[server] or [client] out'), <>Output directory, relative to the member. Default: <code>src/generated</code> (node, typescript), <code>src/main/java</code> (java), <code>generated</code> (rust). Two targets must not share one (<code>E0703</code>), and it must not be the workspace root or a contracts directory (<code>E0704</code>).</>],
          [c('[server] or [client] framework'), <>Optional framework, validated by the emitter for the role. Unknown is <code>E0803</code>, and the message lists the valid values.</>],
          [c('[server] clients'), <>Servers: <code>false</code> turns off the generated clients of consumed services.</>],
          [c('[server] or [client] app'), <>The app that consumes the output, for <code>veld setup</code> (relative to the member&apos;s <code>veld.toml</code>).</>],
          [c('[server.options], [client.options]'), 'Emitter-specific options, validated against the emitter (unknown key: E0802). See the next table.'],
          [c('[plugins.<name>] command'), <>Root file only. The plugin executable, resolved on <code>PATH</code> or as a relative path.</>],
          [c('[plugins.<name>] version'), 'The plugin version.'],
          [c('[plugins.<name>] entry'), <>Optional script that an interpreter <code>command</code> runs. Veld appends it to <code>command</code> unless already present.</>],
          [c('[plugins.<name>] sha256'), <>Hash of <code>entry</code> if set, else of the resolved executable. A mismatch is refused (<code>E0705</code>). For a script plugin always set <code>entry</code>.</>],
          [c('[registry] url'), <>Root file only. The registry. <code>--registry</code> and <code>VELD_REGISTRY</code> override it. Must be https, except localhost.</>],
          [c('[dependencies]'), <>Root file only. Registry packages: <code>"@org/name" = "^1.2"</code>, resolved and pinned in <code>veld.lock</code>.</>],
          [c('[lint], [hooks]'), 'Root file only.'],
        ]}
      />
      <Info>
        <code>[workspace]</code>, <code>[registry]</code>, <code>[dependencies]</code>, <code>[plugins.*]</code>,{' '}
        <code>[lint]</code> and <code>[hooks]</code> belong in the root file. In a member file each is <code>E0711</code>,
        and so is a <code>[server]</code> in an app.
      </Info>

      <Sub>Frameworks and options per emitter</Sub>
      <Table
        head={['Emitter', 'Role', 'framework values (versions)', 'Options']}
        rows={[
          [c('node'), 'server', <>{c('express')}, {c('fastify')}, {c('hono')}, {c('node-http')} (unversioned), {c('nest')} ({c('"10"')})</>, <><code>typescript</code>, <code>framework-version</code>, <code>package-name</code>, <code>testing</code></>],
          [c('typescript'), 'client', <>none (plain client), {c('react')} ({c('"18"')}), {c('vue')} ({c('"3"')})</>, <><code>typescript</code>, <code>framework-version</code>, <code>package-name</code></>],
          [c('java'), 'server', <>{c('jdk')} (unversioned), {c('spring')} ({c('"3"')})</>, <><code>package</code>, <code>java</code>, <code>framework-version</code>, <code>testing</code></>],
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
          [c('testing'), 'node, java, rust (servers)', <><code>true</code> also writes a test handler for each service with endpoint rules, for <code>veld test --live</code>. It must never be mounted in production (see <a href="#rules-testing">Examples and veld test</a>)</>, <code>false</code>],
          [c('query-options'), 'typescript', <><code>true</code> writes <code>queries.ts</code>: query keys and query options as plain objects, no dependency</>, <code>false</code>],
        ]}
      />
      <List>
        <li>An unknown language version is <code>E0802</code>. A <code>framework-version</code> outside the adapter's list, one without a <code>framework</code>, or one for an unversioned framework is <code>E0803</code>. A language older than the framework needs is <code>E0803</code>. All are reported before generating, with a message that lists the valid values.</li>
        <li>The list of frameworks comes from each emitter's adapter registry, so this table, the <code>E0803</code> message and the generated adapters cannot drift apart.</li>
      </List>
      <CodeBlock title="A Spring server on Java 21 (services/orders/veld.toml)" lang="toml">{`[server]
emitter = "java"
framework = "spring"
out = "src/main/java"

[server.options]
java = "21"
framework-version = "3"
package = "com.acme.orders"`}</CodeBlock>

      <Sub>Rules</Sub>
      <List>
        <li>A single file with one <code>[service]</code> is a complete project: the project itself is that one service.</li>
        <li>Generated output never contains absolute paths, user names, host names or timestamps.</li>
        <li>The base URL of a client is resolved from the constructor, then <code>VELD_&lt;SERVICE&gt;_URL</code>, then <code>base-url</code>.</li>
      </List>

      <Sub>Generated files next to the root veld.toml</Sub>
      <Table
        head={['File', 'What']}
        rows={[
          [c('veld.lock.ir.json'), <>Written by a full <code>veld generate</code> (every member and target): the compiled IR as pretty, sorted JSON. A snapshot of the contract as last generated. <strong>Commit it.</strong> <code>veld diff</code> compares against it.</>],
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
