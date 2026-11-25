export interface NavGroup {
  group: string;
  items: { id: string; label: string }[];
}

/** The docs sidebar. Every id is the anchor of a section or sub-heading in a section component. */
export const nav: NavGroup[] = [
  {
    group: 'Getting started',
    items: [
      { id: 'getting-started', label: 'Overview' },
      { id: 'try-browser', label: 'Try it in the browser' },
      { id: 'install', label: 'Install' },
      { id: 'init', label: 'veld init' },
      { id: 'first-contract', label: 'A first contract' },
      { id: 'generate', label: 'veld generate' },
      { id: 'setup', label: 'veld setup' },
    ],
  },
  {
    group: 'The contract language',
    items: [
      { id: 'language', label: 'Overview' },
      { id: 'lang-models', label: 'Models' },
      { id: 'lang-enums', label: 'Enums' },
      { id: 'lang-unions', label: 'Unions' },
      { id: 'lang-errors', label: 'Errors' },
      { id: 'lang-services', label: 'Services and endpoints' },
      { id: 'lang-streaming', label: 'Streaming' },
      { id: 'lang-imports', label: 'Imports and packages' },
      { id: 'lang-annotations', label: 'Annotations and constraints' },
      { id: 'lang-prelude', label: 'Prelude errors' },
      { id: 'lang-generics', label: 'Generics' },
    ],
  },
  {
    group: 'Business rules',
    items: [
      { id: 'rules', label: 'Overview' },
      { id: 'rules-invariants', label: 'Invariants' },
      { id: 'rules-endpoint', label: 'Endpoint rules' },
      { id: 'rules-output', label: 'Output mode and hook' },
      { id: 'rules-states', label: 'State machines' },
      { id: 'rules-policies', label: 'Policies' },
      { id: 'rules-testing', label: 'Examples and veld test' },
      { id: 'rules-tools', label: 'veld diff and OpenAPI' },
    ],
  },
  {
    group: 'Using generated code',
    items: [
      { id: 'using', label: 'Overview' },
      { id: 'using-server', label: 'The server' },
      { id: 'using-adapters', label: 'Server adapters' },
      { id: 'using-client', label: 'The TypeScript client' },
      { id: 'using-react', label: 'React hooks' },
      { id: 'using-vue', label: 'Vue composables' },
      { id: 'using-java', label: 'Java server and client' },
      { id: 'using-streaming', label: 'Streaming' },
    ],
  },
  {
    group: 'Microservices',
    items: [
      { id: 'microservices', label: 'Workspaces and consumes' },
      { id: 'ms-clients', label: 'Service clients' },
      { id: 'ms-base-urls', label: 'Base URLs' },
      { id: 'ms-libraries', label: 'Shared libraries' },
    ],
  },
  {
    group: 'Errors and equality',
    items: [
      { id: 'errors', label: 'Problem+json' },
      { id: 'signatures', label: 'Signatures' },
      { id: 'equality', label: 'same() and matches()' },
    ],
  },
  {
    group: 'Reference',
    items: [
      { id: 'config', label: 'veld.toml' },
      { id: 'cli', label: 'CLI' },
      { id: 'diagnostics', label: 'Diagnostics' },
    ],
  },
  {
    group: 'Ecosystem',
    items: [
      { id: 'registry', label: 'Registry' },
      { id: 'editors', label: 'Editors' },
      { id: 'extending', label: 'Extending Veld' },
    ],
  },
];
