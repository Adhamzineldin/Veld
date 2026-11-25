import { useState } from 'react';
import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';
import { REPO, styles } from './ctx';

const installTabs: Record<string, { label: string; cmd: string; note?: string }> = {
  source: {
    label: 'From source',
    cmd: `git clone ${REPO}\ncd Veld\ncargo build -p veld --release       # produces target/release/veld, one self-contained binary`,
    note: 'Needs a Rust toolchain. The result is a single binary with no other runtime.',
  },
  npm: { label: 'npm', cmd: 'npm install -g @maayn/veld' },
  pip: { label: 'pip', cmd: 'pip install maayn-veld' },
  brew: { label: 'Homebrew', cmd: 'brew install maayn-veld/tap/maayn-veld' },
};

export default function GettingStarted() {
  const [tab, setTab] = useState('source');
  const current = installTabs[tab];

  return (
    <>
      <Section id="getting-started" title="Getting started">
        <P>
          <strong>Veld</strong> is a contract-first API code generator. You describe your API in a small
          language (models, enums, errors, services) in <code>.veld</code> files. Veld compiles the
          contract and generates a typed server, a typed client, or both. Version 2.0 is a rewrite in Rust
          with a new language and a new project file, <code>veld.toml</code>.
        </P>
        <Info>
          <strong>Three guarantees.</strong> Generated code has <strong>no runtime dependencies</strong> (a
          framework adapter may import the framework you chose). Output is <strong>deterministic</strong>:
          the same contract gives the same bytes. And generation is <strong>never destructive</strong>:
          Veld only writes inside a target's <code>out</code> directory, and never overwrites a file you
          edited.
        </Info>
        <P>
          The current release is <code>2.0.0-beta.1</code>. The specs in <code>spec/</code> are normative.
        </P>
      </Section>

      <Section id="try-browser" title="Try it in the browser">
        <P>
          The <a href="/playground">playground</a> runs the real Veld compiler and generators in your browser, compiled to
          WebAssembly. Edit a contract, pick a target (Node server, TypeScript client, Java, Rust or OpenAPI) and read the
          generated files, with diagnostics and their codes inline. The output is byte for byte what <code>veld generate</code>{' '}
          writes, and you can download it as a zip. Nothing is uploaded, and it works offline once loaded.
        </P>
      </Section>
      <Section id="install" title="Install">
        <P>
          Veld is one self-contained binary. Build it from source, or use a wrapper that downloads the
          release binary for your platform and verifies its SHA256 (the wrappers work once a release is
          published).
        </P>
        <div className={styles.installTabs}>
          {Object.entries(installTabs).map(([key, t]) => (
            <button
              key={key}
              className={`${styles.installTab} ${tab === key ? styles.installTabActive : ''}`}
              onClick={() => setTab(key)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <CodeBlock title={current.label}>{current.cmd}</CodeBlock>
        {current.note && <P>{current.note}</P>}
        <CodeBlock title="Check it works">{`veld --version\nveld 2.0.0-beta.1`}</CodeBlock>
      </Section>

      <Section id="init" title="veld init">
        <P>
          <code>veld init</code> creates <code>veld.toml</code> and a starter contract. On a terminal with
          no flags it asks a few questions: the project name, a single service or a workspace, the server
          targets, the client target and the output folders. Flags answer the same questions without a
          terminal, and <code>--yes</code> takes the default for everything you did not give.
        </P>
        <CodeBlock title="Non-interactive">{`veld init --name shop --server node:express --client typescript:react --yes`}</CodeBlock>
        <Table
          head={['Flag', 'Meaning']}
          rows={[
            [<code>--name &lt;NAME&gt;</code>, 'Project name (default: the directory name)'],
            [<code>--workspace [&lt;SERVICES&gt;]</code>, 'Scaffold a microservices workspace, optionally with comma-separated service names'],
            [<code>--server &lt;KIND[:FRAMEWORK]&gt;</code>, <>Server target: <code>node[:express|fastify|hono|nest|node-http]</code>, <code>java[:jdk|spring]</code> or <code>none</code>. Prefix with <code>service=</code> to set one service of a workspace</>],
            [<code>--client &lt;KIND[:FRAMEWORK]&gt;</code>, <>Client target: <code>typescript[:react|vue]</code>, <code>java</code> or <code>none</code></>],
            [<code>--base-url &lt;SERVICE=URL&gt;</code>, 'Base URL of a workspace service (repeatable). A workspace asks for it per service'],
            [<code>--consumes &lt;SERVICE=A,B&gt;</code>, 'Services a workspace service calls (repeatable). Nothing is consumed unless you say so'],
            [<code>-y, --yes</code>, 'Use the defaults for everything not given as a flag; never prompt'],
          ]}
        />
        <P>That command writes the following.</P>
        <CodeBlock title="veld.toml" lang="toml">{`[project]
name = "shop"
contracts = "contracts"

[[targets]]
name = "api"
role = "server"
emitter = "node"
framework = "express"
service = "shop"
out = "generated/server"

[[targets]]
name = "web"
role = "client"
emitter = "typescript"
framework = "react"
out = "generated/client"`}</CodeBlock>
      </Section>

      <Section id="first-contract" title="A first contract">
        <P>
          This is the contract of the <code>examples/hello</code> project: one model, one service, one
          endpoint. <code>NotFound</code> is a prelude error, so it needs no declaration.{' '}
          <code>@auth(none)</code> says the service is public: <code>veld check</code> warns about an
          endpoint that does not say who may call it.
        </P>
        <CodeBlock title="contracts/hello.veld" lang="veld">{`/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
@auth(none)
service Hello at "/hello" {
  /// Greets the person with the given name.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}`}</CodeBlock>
        <P>
          <code>veld check</code> compiles every contract and reports diagnostics, with a stable code for
          each one (see <a href="#diagnostics">Diagnostics</a>).
        </P>
        <CodeBlock title="Terminal">{`$ veld check
✓ 1 service, 1 endpoint, 1 type`}</CodeBlock>
      </Section>

      <Section id="generate" title="veld generate">
        <P>
          <code>veld generate</code> writes the code for every target, or for one target by name
          (<code>veld generate web</code>). <code>--dry-run</code> shows what would change and writes
          nothing, and <code>--watch</code> regenerates whenever a <code>.veld</code> file or{' '}
          <code>veld.toml</code> changes.
        </P>
        <CodeBlock title="Terminal">{`$ veld generate
✓ api -> generated/server: 7 created, 0 updated, 0 deleted, 0 unchanged
✓ web -> generated/client: 8 created, 0 updated, 0 deleted, 0 unchanged
hint: 2 target(s) are not wired into your app yet; run \`veld setup\``}</CodeBlock>
        <Sub>Generation never destroys your work</Sub>
        <List>
          <li>Every write goes through a staged writer with a manifest (<code>.veld-manifest.json</code>) and path containment.</li>
          <li>A file that Veld did not write is never touched. It is reported as a conflict.</li>
          <li>A file Veld wrote that you then edited is <strong>user-modified</strong>: <code>generate</code> refuses to overwrite or delete it and names it. <code>--force</code> overrides.</li>
          <li><code>veld clean</code> removes the generated files you have not modified and keeps the rest.</li>
          <li>A full generate also writes <code>veld.lock.ir.json</code>, a snapshot of the contract. Commit it: <code>veld diff</code> compares against it.</li>
        </List>
      </Section>

      <Section id="setup" title="veld setup">
        <P>
          Each <code>node</code> and <code>typescript</code> target is an npm package named{' '}
          <code>@veld/&lt;target name&gt;</code> with a <code>package.json</code> and no dependencies. Your
          code imports <code>@veld/api</code>, never a relative path into <code>src/generated</code>.{' '}
          <code>veld setup</code> wires each target into the app that consumes it.
        </P>
        <CodeBlock title="Terminal">{`$ veld setup --dry-run
would change package.json
  + dependencies["@veld/api"] = "file:generated/server"  (api)
  + dependencies["@veld/web"] = "file:generated/client"  (web)
dry run: nothing was written

$ veld setup
$ npm install        # in each app`}</CodeBlock>
        <Table
          head={['Target', 'What setup edits']}
          rows={[
            ['TypeScript', <>Adds <code>"@veld/&lt;name&gt;": "file:&lt;relative out&gt;"</code> to the app's <code>dependencies</code>. A <code>file:</code> dependency works for <code>tsc</code>, Node, Vite and test runners alike.</>],
            ['Maven', <>Adds a <code>build-helper-maven-plugin</code> <code>add-source</code> execution for <code>out</code> (nothing to do when <code>out</code> is <code>src/main/java</code>).</>],
            ['Gradle', <>Appends a <code>// veld:begin</code> ... <code>// veld:end</code> block with <code>sourceSets.main.java.srcDir(...)</code>.</>],
          ]}
        />
        <P>
          The app is the target's <code>app = "path"</code> key, or else the nearest ancestor of{' '}
          <code>out</code> that has a <code>package.json</code>, <code>pom.xml</code> or{' '}
          <code>build.gradle(.kts)</code>. Edits keep the file's formatting, are idempotent and never touch
          unrelated keys. When a file cannot be edited safely, setup prints the exact snippet to paste.
        </P>
      </Section>
    </>
  );
}
