import { CodeBlock, Section, Sub, P, Info, Warn, List } from './shared';

export default function Registry() {
  return (
    <Section id="registry" title="Registry">
      <P>
        The registry shares contracts as versioned packages named <code>@org/name</code>. It is a separate
        HTTP service (the Go server in <code>registry/</code>) that stores packages as tarballs and validates
        the manifest on publish. The CLI talks to it with <code>login</code>, <code>add</code>,{' '}
        <code>publish</code> and <code>install</code>.
      </P>

      <Sub>Log in</Sub>
      <CodeBlock title="Terminal">{`veld login --registry https://registry.example.com --token vtk_...
veld logout`}</CodeBlock>
      <P>
        The token is read from stdin when <code>--token</code> is omitted. Tokens are stored per registry URL
        in <code>~/.veld/credentials.toml</code> (<code>$VELD_HOME</code> overrides the directory) and are never
        printed. The registry URL comes from <code>--registry</code>, then <code>[registry]</code> in{' '}
        <code>veld.toml</code>, then <code>$VELD_REGISTRY</code>, then the last login. It must be{' '}
        <code>https</code>, except for <code>localhost</code> and <code>127.0.0.1</code> (or pass{' '}
        <code>--allow-insecure</code>).
      </P>

      <Sub>Publish</Sub>
      <P>
        Give the <code>[service]</code> or <code>[library]</code> table of the member a{' '}
        <code>package</code>, a semver <code>version</code> and optionally a <code>description</code>.{' '}
        <code>veld publish</code> packs <code>contracts/**.veld</code> and a <code>veld-package.toml</code>{' '}
        (<code>name</code>, <code>version</code>, <code>language = "2"</code> and a <code>[files]</code> table of
        SHA-256 checksums) into a deterministic tar.gz and uploads it.
      </P>
      <CodeBlock title="libs/money/veld.toml, and [dependencies] in the root veld.toml" lang="toml">{`[library]
package = "@acme/money"
version = "1.0.0"
description = "Shared money types"

# in the veld.toml of the workspace root:
[dependencies]
"@acme/billing" = "^1.2"`}</CodeBlock>
      <CodeBlock title="Terminal">{`veld publish --dry-run                  # pack and check, upload nothing
veld publish --package money            # required in a workspace with several packages`}</CodeBlock>

      <Sub>Add and install</Sub>
      <CodeBlock title="Terminal">{`veld add @acme/billing           # or @acme/billing@^1.2
veld install                     # exactly what veld.lock pins`}</CodeBlock>
      <List>
        <li><code>veld add</code> writes <code>[dependencies]</code>, resolves the whole graph, installs it and pins it in <code>veld.lock</code>. It updates <code>veld.toml</code>, <code>veld.lock</code> and the cache as a unit: on any failure nothing is changed.</li>
        <li><code>veld install</code> installs exactly the locked graph, direct and transitive, and verifies every checksum. A mismatch is a hard error.</li>
        <li><code>veld check</code> and <code>veld generate</code> install what <code>veld.lock</code> pins and the cache lacks. Without a lock they never touch the network.</li>
        <li>Installed packages live in <code>.veld/packages/@org/name/&lt;version&gt;/</code> (gitignored).</li>
      </List>
      <CodeBlock title="Using a package" lang="veld">{`use @acme/billing:invoices.{Invoice}`}</CodeBlock>

      <Sub id="registry-lock">veld.lock</Sub>
      <P>
        <code>veld.lock</code> is TOML, sorted by name and deterministic. Each <code>[[package]]</code> entry has{' '}
        <code>name</code>, <code>version</code>, the <code>sha256</code> of the tarball, the <code>registry</code> and{' '}
        <code>dependencies</code>, the sorted names of the packages it depends on. Every package in the graph
        has an entry, so the lock is closed: a dependency that is not locked makes the lock invalid. Commit it.
        It is distinct from <code>veld.lock.ir.json</code>, the contract snapshot.
      </P>

      <Sub id="registry-transitive">Transitive dependencies</Sub>
      <P>
        A package's own <code>[dependencies]</code> are written to its <code>veld-package.toml</code> by{' '}
        <code>veld publish</code>. The registry validates names and ranges at publish time and returns them with
        each version listing, so resolving never downloads a package.
      </P>
      <List>
        <li><strong>One version per package.</strong> The resolver picks the highest version that satisfies every range in the graph, and keeps the version already pinned when it still fits, so <code>veld add</code> does not upgrade unrelated packages. Two versions are never installed side by side, so <code>use @org/name:...</code> is unambiguous. Deprecated versions are skipped.</li>
        <li><strong>Conflicts</strong> are a hard error that shows the dependency paths, for example <code>veld.toml → @acme/billing@1.0.0 → @acme/money@^1</code> next to <code>veld.toml → @acme/payroll@1.0.0 → @acme/money@^2</code>. Nothing is changed.</li>
        <li><strong>Cycles</strong> between packages are legal and resolve as long as the versions are consistent. An inconsistent cycle is reported as a conflict marked <code>(dependency cycle)</code>.</li>
      </List>

      <Sub id="registry-e0206">E0206: visibility</Sub>
      <P>
        The project may <code>use</code> only the packages listed in its own <code>[dependencies]</code>, and a
        registry package may <code>use</code> only the packages in its own <code>[dependencies]</code>. Importing
        a package that is merely installed because something else needs it is <code>E0206</code>.
      </P>
      <Warn>
        <strong>E0206</strong> comes with a help line saying where to declare the dependency. If your contract
        imports <code>@acme/money</code> that arrives only through <code>@acme/billing</code>, run{' '}
        <code>veld add @acme/money</code> so it is a direct dependency.
      </Warn>
      <Info>
        Self-hosting: the registry server needs PostgreSQL, a JWT secret and a storage directory. See{' '}
        <code>registry/README.md</code> in the repository.
      </Info>
    </Section>
  );
}
