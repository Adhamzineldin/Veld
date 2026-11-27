import { CodeBlock, Section, Sub, P, Info } from './shared';
import { cliHelp, cliVersion } from './cliHelp';

interface Cmd {
  key: string;
  about: string;
}

const commands: Cmd[] = [
  { key: 'veld init', about: 'Create veld.toml and a starter contract. Interactive on a terminal; flags and --yes skip the questions.' },
  { key: 'veld check', about: 'Compile every contract and report diagnostics.' },
  { key: 'veld generate', about: 'Write code for every target, or one by name. Never overwrites a file you edited unless --force. In a workspace it works on the member whose directory you are in; -p NAME picks members, --workspace takes all of them, and --affected --since REF takes the members a change can have affected. The same selection applies to watch, clean, setup and test.' },
  { key: 'veld clean', about: 'Remove generated files you have not modified. Modified files are kept and stay listed in the manifest.' },
  { key: 'veld setup', about: 'Wire generated targets into your apps: @veld/<target> file: dependencies, or a Maven or Gradle source root.' },
  { key: 'veld fmt', about: 'Format .veld files canonically. --check changes nothing and exits 1 if a file would change.' },
  { key: 'veld diff', about: 'Show breaking and non-breaking contract changes against veld.lock.ir.json or a git ref, rules included. A changed rule expression is possibly breaking. Use --strict in CI: it fails on those too.' },
  { key: 'veld test', about: 'Run the example blocks of the contracts. Offline by default; --live <url> sends them to a test handler, --dry-run stops before your handler, --fuzz N --seed S adds generated requests. --coverage [--fail-under N] reports which rules the examples hold and violate; --from-violation <file> turns a captured violation into an example block.' },
  { key: 'veld export', about: 'Export the contracts as OpenAPI 3.1, the rule book as Markdown, or the compiled IR.' },
  { key: 'veld export rules', about: 'The rules of the workspace as Markdown, for people who do not read contracts (see spec/rules.md section 19.2).' },
  { key: 'veld export openapi', about: 'OpenAPI 3.1 JSON for the whole workspace, or one service.' },
  { key: 'veld export ir', about: 'The compiled IR as JSON, for plugin authors (see spec/ir.md).' },
  { key: 'veld plugin', about: 'Create and test emitter plugins.' },
  { key: 'veld plugin new', about: 'Scaffold a plugin project in ./<name> (a TypeScript project by default).' },
  { key: 'veld plugin test', about: 'Run the protocol conformance suite against a plugin command. Exits 1 on any failure.' },
  { key: 'veld add', about: 'Add a registry package to veld.toml, resolve the whole graph and pin it in veld.lock.' },
  { key: 'veld install', about: 'Install exactly the packages pinned in veld.lock, verifying every checksum.' },
  { key: 'veld publish', about: 'Pack this project\'s library or service and upload it to the registry.' },
  { key: 'veld login', about: 'Store a registry API token (read from stdin when --token is omitted).' },
  { key: 'veld logout', about: 'Forget the stored registry token.' },
];

const idOf = (key: string) => 'cli-' + key.replace(/^veld ?/, '').replace(/ /g, '-');

export default function CliReference() {
  return (
    <Section id="cli" title="CLI reference">
      <P>
        The text below is the real <code>--help</code> output of <code>{cliVersion}</code>, copied by a
        script, so it cannot drift from the binary. Every command also takes the global options{' '}
        <code>--config &lt;PATH&gt;</code>, <code>-q, --quiet</code> and <code>--no-color</code>.
      </P>
      <Info>
        Two more commands exist but are hidden from the help: <code>veld lsp</code> starts the language
        server that the editor plugins talk to, and <code>veld debug</code> is for Veld developers.
      </Info>
      <CodeBlock title="veld --help">{cliHelp['veld']}</CodeBlock>
      {commands.map((c) => (
        <div key={c.key}>
          <Sub id={idOf(c.key)}>
            <code>{c.key}</code>
          </Sub>
          <P>{c.about}</P>
          <CodeBlock title={`${c.key} --help`}>{cliHelp[c.key]}</CodeBlock>
        </div>
      ))}
    </Section>
  );
}
