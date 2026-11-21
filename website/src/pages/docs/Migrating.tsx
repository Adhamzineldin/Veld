import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';
import { REPO } from './ctx';

const v1Contract = `model User {
  description: "A registered user"
  id:    uuid
  email: string
  role:  Role   @default(user)
}

module Users {
  prefix: /api/v1

  action GetUser {
    method: GET
    path:   /users/:id
    output: User
    errors: [NotFound]
  }
}`;

const v2Contract = `/// A registered user
model User {
  id: uuid
  email: string
  role: Role = .user
}

service Users at "/api/v1" {
  get "/users/{id}" getUser(id: uuid) -> User | NotFound
}`;

const k = (s: string) => <code>{s}</code>;

export default function Migrating() {
  return (
    <Section id="migrating" title="Migrating from v1">
      <P>
        Veld 2.0 has a new language and a new project file. The v1 Go implementation is frozen in{' '}
        <code>archive/v1/</code> at the git tag <code>v1-final</code>. v1 syntax is error <code>E0001</code>,
        with a hint to run <code>veld migrate</code>.
      </P>
      <Sub>Steps</Sub>
      <List>
        <li>Tag or keep your v1 project in git. <code>veld migrate</code> never modifies v1 files.</li>
        <li>Run <code>veld migrate</code> in the v1 project. Use <code>--from &lt;dir&gt;</code> if the v1 config is elsewhere, <code>--out &lt;dir&gt;</code> for the contracts directory (default <code>contracts</code>), and <code>--dry-run</code> to preview.</li>
        <li>Read the warnings. Constructs without a v2 equivalent are reported, and where they have a place in a file they also appear as a <code>// TODO(veld migrate): ...</code> comment. Backends and frameworks without a v2 emitter are written as commented-out targets.</li>
        <li>Run <code>veld check</code>, fix what it reports, run <code>veld fmt</code>, then <code>veld generate</code>.</li>
        <li>Replace the generated code in your application. The generated API is different: see <a href="#using">Using generated code</a>.</li>
      </List>
      <CodeBlock title="Terminal">{`veld migrate --from path/to/v1/project   # writes veld.toml and contracts/
veld check
veld fmt
veld generate`}</CodeBlock>
      <Info>
        An example of the result is <code>examples/microservices</code>, migrated from{' '}
        <code>archive/v1/examples/microservices</code> and then rewritten in idiomatic v2.
      </Info>

      <Sub>The same contract in v1 and v2</Sub>
      <CodeBlock title="v1">{v1Contract}</CodeBlock>
      <CodeBlock title="v2" lang="veld">{v2Contract}</CodeBlock>

      <Sub>Syntax changes</Sub>
      <Table
        head={['v1', 'v2']}
        rows={[
          [k('module Users { prefix: /api }'), k('service Users at "/api" { ... }')],
          [k('action GetUser { method: GET path: /users/:id }'), k('get "/users/{id}" getUser(id: uuid)')],
          [k('input: CreateUser'), <>{k('body: CreateUser')} (or {k('query: Filter')} for a query object)</>],
          [k('output: User'), k('-> User')],
          [k('errors: [NotFound, Conflict]'), <>{k('-> User | NotFound | Conflict')}. Errors are declarations: {k('error OutOfStock 409 { sku: string }')}</>],
          [k('description: "text"'), <>{k('///')} doc comment</>],
          [k('field: Type @default(x)'), <>{k('field: Type = x')}. Enum defaults are {k('= .variant')}</>],
          [k('enum Role { admin user }'), <>{k('enum Role { admin, user }')} (commas optional). New: {k('= "WIRE"')} overrides, and {k('union')}</>],
          [k('import /models/*'), <>{k('use users.{User, Role}')} (named imports, no globs)</>],
          [k('import @alias/x'), <>{k('use iam:users.{User}')}, or {k('use @org/name:mod.{X}')} for a registry package</>],
          [<>{k('method: WS')} and {k('stream:')}</>, <>{k('ws "/path" name(body: stream In) -> stream Out')}, and {k('get ... -> stream T')} for SSE</>],
          [k('middleware: [Auth]'), <>{k('@auth(bearer)')} or {k('@middleware(Name)')}</>],
          [k('name?: string'), <>Unchanged: {k('name?: string')}</>],
        ]}
      />

      <Sub>Other things that changed</Sub>
      <Table
        head={['Area', 'v1', 'v2']}
        rows={[
          ['Config', <code>veld.config.json</code>, <><code>veld.toml</code> with <code>[[targets]]</code>, each with a <code>role</code>, an <code>emitter</code> and an <code>out</code></>],
          ['Binary', 'A Go binary', 'One self-contained Rust binary'],
          ['Commands', <><code>validate</code>, <code>ast</code>, <code>watch</code>, <code>ci</code>, <code>docs</code>, <code>graphql</code>, <code>schema</code>, <code>openapi</code></>, <><code>check</code>, <code>generate --watch</code>, <code>export openapi</code>, <code>export ir</code>. The rest are gone</>],
          ['Lock file', <><code>.veld.lock.json</code>, <code>.veld-cache.json</code></>, <><code>veld.lock.ir.json</code> (and <code>veld.lock</code> for registry packages)</>],
          ['Import of generated code', <code>@veld/generated</code>, <><code>@veld/&lt;target name&gt;</code>, wired by <code>veld setup</code></>],
          ['Errors', 'Codes listed per action', <>Declarations with identity. On the wire: <code>application/problem+json</code>, and <code>long</code> is a decimal string. Matched by signature</>],
          ['Backends', 'Node, Python, Go, Java, C#, PHP, Rust', 'Built in: Node (server), Java (server and client). Others: write an emitter plugin'],
          ['Frontends', 'TypeScript, React, Dart, Kotlin, Swift, Svelte, Vue, Angular', 'Built in: TypeScript, with React hooks or Vue composables, and Java. Others: write an emitter plugin'],
        ]}
      />
      <P>
        The Go, NuGet, Chocolatey, apt and Composer distribution channels are not maintained for 2.0. See the{' '}
        <a href={`${REPO}/blob/master/CHANGELOG.md`}>changelog</a> for the full list of breaking changes.
      </P>
    </Section>
  );
}
