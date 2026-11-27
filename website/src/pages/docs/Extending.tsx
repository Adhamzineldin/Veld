import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';

export default function Extending() {
  return (
    <Section id="extending" title="Extending Veld">
      <P>
        An emitter turns the IR into files. The IR is the only thing a generator, a tool or a plugin sees.
        There are two kinds of emitter, and both obey the same rules: a plugin is not a lesser citizen, because
        the host enforces the paths, limits and capabilities.
      </P>
      <Table
        head={['', 'Built-in emitter', 'Plugin']}
        rows={[
          ['Language', 'Rust, a crate in crates/emitters/', 'Anything that reads and writes JSON lines (the scaffold uses TypeScript)'],
          ['Ships', 'Inside the veld binary', 'As a separate executable pinned in veld.toml by sha256'],
          ['Choose it when', 'The target is first-party, or must be fast and always available', 'The target is yours, experimental, or best written in its own language'],
          ['Contract', 'The Generator trait', 'spec/plugin-protocol.md (IR types in spec/ir.d.ts)'],
          ['Checked by', 'veld-emit-conformance, snapshots, toolchain tests', 'veld plugin test'],
        ]}
      />

      <Sub id="extending-rules">The rules every emitter follows</Sub>
      <List>
        <li><code>generate</code> is a pure function from IR to a set of files. It does no I/O: Veld writes the files through the staged writer.</li>
        <li>Output is deterministic: no timestamps, absolute paths or host names.</li>
        <li>Generated code has zero runtime dependencies, except a framework adapter for the framework the user chose.</li>
        <li>Declare only the capabilities you implement. For anything else Veld reports <code>E0801</code>. Never emit wrong or partial code.</li>
        <li>Embed every <code>sig</code> from the IR verbatim, and provide <code>canonical(value)</code> and <code>same(a, b)</code> in the runtime.</li>
      </List>

      <Sub id="extending-plugins">Plugins</Sub>
      <CodeBlock title="Terminal">{`veld plugin new mylang             # scaffolds ./mylang (TypeScript)
cd mylang && npm install && npm run build
veld plugin test node dist/main.js # runs the protocol conformance suite`}</CodeBlock>
      <P>
        The scaffold has <code>src/protocol.ts</code> (handshake and loop), <code>src/ir.d.ts</code> (generated
        IR types), <code>src/generate.ts</code> (your code) and a README. <code>veld plugin test</code> checks the
        handshake, path safety, determinism and signature embedding, and generates once for every declared
        language version and framework. It exits 1 on any failure. Then pin the plugin in the{' '}
        <code>veld.toml</code> of the workspace root, and use it in the <code>veld.toml</code> of the member
        that gets the generated code. A plugin has no default <code>out</code>.
      </P>
      <CodeBlock title="veld.toml (the workspace root, or the only file)" lang="toml">{`[plugins.mylang]
command = "node mylang/dist/main.js"
version = "0.1.0"
sha256 = "<sha-256 of the first word of command>"`}</CodeBlock>
      <CodeBlock title="veld.toml of the member" lang="toml">{`[client]
emitter = "plugin:mylang"
out = "generated/mylang"`}</CodeBlock>
      <P>
        The protocol is one JSON object per line over stdin and stdout, in a fresh empty working directory
        with an empty environment apart from <code>PATH</code>:
      </P>
      <List>
        <li><strong>Handshake</strong>: Veld sends <code>hello</code>. The plugin replies within 5 s with its name, roles, capabilities, and optionally a <code>frameworks</code> and <code>language</code> declaration that Veld checks with the same central rules as for built-ins (<code>E0802</code>, <code>E0803</code>).</li>
        <li><strong>Generate</strong>: Veld sends the role, the IR, the selected services and the options. The plugin replies within 60 s with <code>files</code> and <code>diagnostics</code>.</li>
        <li><strong>Host guarantees</strong>: absolute paths, <code>..</code>, a NUL byte or a backslash in a path, more than 10,000 files, a file over 10 MiB or more than 200 MiB in total are refused (<code>E0707</code>). A timeout or non-zero exit is <code>E0708</code>, and the previous output is left untouched. A hash mismatch is <code>E0705</code>.</li>
      </List>
      <Info>
        <code>veld export ir</code> prints the compiled IR, which is what a plugin receives in{' '}
        <code>generate</code>. Plugins must embed the <code>sig</code> values it contains.
      </Info>

      <Sub id="extending-builtin">Built-in emitters</Sub>
      <CodeBlock title="Terminal">{`cargo xtask new-emitter <name> --roles server,client
cargo test -p veld-emit-<name>
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings`}</CodeBlock>
      <List>
        <li><code>new-emitter</code> creates <code>crates/emitters/&lt;name&gt;</code>, a conformance test and the registry entry.</li>
        <li>Write <code>spec/targets/&lt;name&gt;.md</code> first: type mapping, file layout, runtime API and the capabilities you will support.</li>
        <li>Implement <code>generate</code> starting from an empty <code>Capabilities</code>, and add features one at a time, each with a test.</li>
        <li>A server framework implements <code>FrameworkAdapter</code> and is listed in the emitter's one registry slice, which feeds <code>frameworks(role)</code> and the central <code>E0803</code> check.</li>
        <li>An emitter never depends on <code>veld-compiler</code>. It sees only <code>veld-ir</code>.</li>
      </List>
      <Sub>Required tests</Sub>
      <Table
        head={['Suite', 'What it checks']}
        rows={[
          ['Shared conformance', 'Capability coverage (E0801), every sig embedded verbatim, determinism'],
          ['Snapshots', 'insta snapshots of the fixture contracts. A person reviews changes with cargo insta test --review'],
          ['Toolchain round trip', 'Compile with the real compiler (tsc, javac, ...) and run a client against a server'],
          ['Wire goldens', 'canonical() reproduces tests/wire/ byte for byte'],
          ['Cross-package equality', 'same holds across packages for a shared error and fails for a different service, operation or payload'],
        ]}
      />
    </Section>
  );
}
