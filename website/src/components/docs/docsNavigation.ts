export type DocsNavItem = {
  id: string;
  label: string;
};

export type DocsNavGroup = {
  group: string;
  items: DocsNavItem[];
};

export const docsSidebarSections: DocsNavGroup[] = [
  {
    group: 'Getting Started',
    items: [
      { id: 'overview', label: 'Overview' },
      { id: 'installation', label: 'Installation' },
      { id: 'quickstart', label: 'Quick Start' },
      { id: 'project-structure', label: 'Project Structure' },
    ],
  },
  {
    group: 'Contract Syntax',
    items: [
      { id: 'models', label: 'Models' },
      { id: 'enums', label: 'Enums' },
      { id: 'modules', label: 'Modules & Actions' },
      { id: 'types', label: 'Field Types' },
      { id: 'inheritance', label: 'Inheritance (extends)' },
      { id: 'maps', label: 'Maps' },
      { id: 'imports', label: 'Import System' },
      { id: 'websockets', label: 'WebSockets' },
    ],
  },
  {
    group: 'CLI Reference',
    items: [
      { id: 'cli-overview', label: 'CLI Overview' },
      { id: 'cli-init', label: 'veld init' },
      { id: 'cli-generate', label: 'veld generate' },
      { id: 'cli-validate', label: 'veld validate' },
      { id: 'cli-watch', label: 'veld watch' },
      { id: 'cli-openapi', label: 'veld openapi' },
      { id: 'cli-schema', label: 'veld schema' },
      { id: 'cli-docs', label: 'veld docs' },
      { id: 'cli-diff', label: 'veld diff' },
      { id: 'cli-ast', label: 'veld ast' },
      { id: 'cli-clean', label: 'veld clean' },
    ],
  },
  {
    group: 'Configuration',
    items: [
      { id: 'config-file', label: 'Config File' },
      { id: 'config-fields', label: 'Config Fields' },
      { id: 'config-aliases', label: 'Import Aliases' },
      { id: 'config-detection', label: 'Auto-Detection' },
    ],
  },
  {
    group: 'Generated Output',
    items: [
      { id: 'output-overview', label: 'Output Overview' },
      { id: 'output-node', label: 'Node.js Backend' },
      { id: 'output-python', label: 'Python Backend' },
      { id: 'output-frontend', label: 'Frontend SDK' },
      { id: 'output-schemas', label: 'Validation Schemas' },
      { id: 'output-routes', label: 'Route Handlers' },
    ],
  },
  {
    group: 'Using Generated Code',
    items: [
      { id: 'usage-backend', label: 'Backend Integration' },
      { id: 'usage-frontend', label: 'Frontend SDK Usage' },
      { id: 'usage-path-alias', label: 'Path Aliases' },
    ],
  },
  {
    group: 'Supported Stacks',
    items: [
      { id: 'stacks-backends', label: 'Backend Emitters' },
      { id: 'stacks-frontends', label: 'Frontend Emitters' },
      { id: 'stacks-extras', label: 'Extras' },
    ],
  },
  {
    group: 'Editor Support',
    items: [
      { id: 'editor-vscode', label: 'VS Code Extension' },
      { id: 'editor-jetbrains', label: 'JetBrains Plugin' },
      { id: 'editor-lsp', label: 'LSP Server' },
    ],
  },
  {
    group: 'Cloud Registry',
    items: [
      { id: 'registry-overview', label: 'Overview' },
      { id: 'registry-selfhost', label: 'Self-Hosting' },
      { id: 'registry-login', label: 'Login & Auth' },
      { id: 'registry-push', label: 'Publishing (push)' },
      { id: 'registry-pull', label: 'Installing (pull)' },
      { id: 'registry-teams', label: 'Teams & Orgs' },
      { id: 'registry-tokens', label: 'API Tokens' },
      { id: 'registry-config', label: 'Config Reference' },
    ],
  },
];
