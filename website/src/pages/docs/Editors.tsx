import { CodeBlock, Section, Sub, P, Info, Table, List } from './shared';

export default function Editors() {
  return (
    <Section id="editors" title="Editors">
      <P>
        Both editor plugins are thin clients for the language server that ships inside the binary:{' '}
        <code>veld lsp</code>. Every language feature comes from that one engine, so the two plugins behave
        the same. Neither contains a lexer or parser of its own. The TextMate grammar behind highlighting is
        generated from the compiler's language table with <code>cargo xtask codegen</code>.
      </P>
      <List>
        <li>Diagnostics for the whole project, with codes and related locations</li>
        <li>Hover: docs, kind, signature and resolved type, and method, path, statuses and errors for endpoints</li>
        <li>Go to definition (types, <code>use</code> imports, errors) and find references</li>
        <li>Completion: keywords, HTTP methods, types with auto-import, annotations with argument snippets, enum variants after <code>= .</code></li>
        <li>Outline, rename across files, formatting and semantic tokens</li>
      </List>

      <Sub id="editor-vscode">VS Code</Sub>
      <P>
        Install Veld 2 and make sure <code>veld</code> is on your <code>PATH</code>, or set{' '}
        <code>veld.path</code>. The extension is in <code>editors/vscode</code>.
      </P>
      <Table
        head={['Setting', 'Default', 'Meaning']}
        rows={[
          [<code>veld.path</code>, <code>veld</code>, <>The executable started as <code>veld lsp</code></>],
          [<code>veld.trace.server</code>, <code>off</code>, 'Trace the language server protocol'],
        ]}
      />

      <Sub id="editor-jetbrains">JetBrains</Sub>
      <P>
        The plugin in <code>editors/jetbrains</code> works in IntelliJ IDEA, RustRover, WebStorm and the other
        2024.3+ IDEs. It runs <code>veld lsp</code> through the LSP4IJ plugin, which the IDE installs with it as
        a declared dependency. Highlighting comes from the same TextMate grammar.
      </P>
      <P>
        Settings | Tools | Veld: <strong>Path to veld</strong>. Empty means look up <code>veld</code> on{' '}
        <code>PATH</code>. If the executable cannot be found, a notification explains it and links to the
        setting. A change applies the next time the language server starts.
      </P>
      <CodeBlock title="Build the plugin">{`cd editors/jetbrains
./gradlew buildPlugin     # build/distributions/veld-jetbrains-2.0.0.zip
./gradlew runIde          # a sandbox IDE with the plugin`}</CodeBlock>
      <Info>
        Any other editor with LSP support can use the same server: run <code>veld lsp</code> as the language
        server command for <code>.veld</code> files.
      </Info>
    </Section>
  );
}
