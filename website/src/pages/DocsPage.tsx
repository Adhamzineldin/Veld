import { useState, useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { Menu, X } from 'lucide-react';
import { highlightVeld, highlightTS } from '../components/SyntaxHighlighter';
import styles from './DocsPage.module.css';

const sidebarSections = [
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

function CodeBlock({ title, children, lang }: { title?: string; children: string; lang?: 'veld' | 'ts' | 'bash' | 'json' }) {
  const rendered = lang === 'veld'
    ? highlightVeld(children)
    : lang === 'ts'
    ? highlightTS(children)
    : null;

  return (
    <div className={styles.codeBlock}>
      {title && (
        <div className={styles.codeHeader}>
          <span className={styles.codeDot} style={{ background: '#f85149' }} />
          <span className={styles.codeDot} style={{ background: '#f0883e' }} />
          <span className={styles.codeDot} style={{ background: '#3fb950' }} />
          <span className={styles.codeHeaderTitle}>{title}</span>
        </div>
      )}
      <pre className={styles.codeContent}>
        {rendered ?? children}
      </pre>
    </div>
  );
}

export default function DocsPage() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [activeId, setActiveId] = useState('overview');
  const [installTab, setInstallTab] = useState('npm');
  const location = useLocation();

  // Scroll to hash on load
  useEffect(() => {
    if (location.hash) {
      const el = document.getElementById(location.hash.slice(1));
      if (el) el.scrollIntoView({ behavior: 'smooth' });
    }
  }, [location.hash]);

  // Track active section on scroll
  useEffect(() => {
    const ids = sidebarSections.flatMap((s) => s.items.map((i) => i.id));
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
            break;
          }
        }
      },
      { rootMargin: '-80px 0px -60% 0px', threshold: 0 }
    );
    for (const id of ids) {
      const el = document.getElementById(id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, []);

  const installCommands: Record<string, { install: string; run: string }> = {
    npm: { install: 'npm install -g @maayn/veld', run: 'npx @maayn/veld generate' },
    yarn: { install: 'yarn global add @maayn/veld', run: 'veld generate' },
    pnpm: { install: 'pnpm add -g @maayn/veld', run: 'veld generate' },
    pip: { install: 'pip install maayn-veld', run: 'veld generate' },
    brew: { install: 'brew install maayn-veld/tap/maayn-veld', run: 'veld generate' },
    go: { install: 'go install github.com/Adhamzineldin/Veld/cmd/veld@latest', run: 'veld generate' },
    composer: { install: 'composer global require maayn/veld', run: 'veld generate' },
    binary: { install: '# Download from GitHub Releases:\n# https://github.com/Adhamzineldin/Veld/releases\n# Extract and add to your PATH', run: 'veld generate' },
  };

  return (
    <div className={styles.docsLayout}>
      {/* Mobile overlay */}
      <div
        className={`${styles.mobileOverlay} ${sidebarOpen ? styles.mobileOverlayOpen : ''}`}
        onClick={() => setSidebarOpen(false)}
      />

      {/* Sidebar */}
      <aside className={`${styles.sidebar} ${sidebarOpen ? styles.sidebarOpen : ''}`}>
        {sidebarSections.map((section) => (
          <div key={section.group} className={styles.sidebarGroup}>
            <div className={styles.sidebarGroupTitle}>{section.group}</div>
            {section.items.map((item) => (
              <a
                key={item.id}
                href={`#${item.id}`}
                className={`${styles.sidebarLink} ${activeId === item.id ? styles.sidebarLinkActive : ''}`}
                onClick={() => setSidebarOpen(false)}
              >
                {item.label}
              </a>
            ))}
          </div>
        ))}
      </aside>

      {/* Mobile sidebar toggle */}
      <button
        className={styles.mobileSidebarToggle}
        onClick={() => setSidebarOpen(!sidebarOpen)}
        aria-label="Toggle docs sidebar"
      >
        {sidebarOpen ? <X size={22} /> : <Menu size={22} />}
      </button>

      {/* Main content */}
      <div className={styles.content}>
        <h1 className={styles.pageTitle}>
          Veld <span className={styles.pageTitleGradient}>Documentation</span>
        </h1>
        <p className={styles.pageSubtitle}>
          Everything you need to know about writing <code>.veld</code> contracts and generating
          typed backends, frontend SDKs, validation, and more.
        </p>

        {/* ─── OVERVIEW ─── */}
        <section id="overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>Overview</h2>
          <p className={styles.sectionDesc}>
            <strong>Veld</strong> is a contract-first, multi-stack API code generator. You write
            <code> .veld</code> contract files describing your models, enums, and API endpoints.
            Veld then generates fully typed backend service interfaces, route wiring with input
            validation, frontend SDKs, OpenAPI specs, database schemas, and more &mdash; for any
            stack you choose.
          </p>
          <div className={styles.infoCard}>
            <strong>Zero runtime dependencies</strong> &mdash; generated code works out of the box
            with no <code>npm install</code> needed (for type-only usage). Validation schemas
            (Zod/Pydantic) are opt-in.
          </div>
          <ul className={styles.featureList}>
            <li>Write your API contract once, generate code for 7+ backend languages and 4+ frontend targets</li>
            <li>Framework agnostic &mdash; works with Express, Fastify, Hono, Flask, and any router with <code>.get()</code>/<code>.post()</code></li>
            <li>Deterministic output &mdash; same input always produces identical output, safe for CI/CD</li>
            <li>Built-in validation with Zod (Node.js) and Pydantic (Python)</li>
            <li>OpenAPI 3.0 spec generation from the same contract</li>
            <li>Watch mode for instant re-generation on file save</li>
            <li>IDE support with VS Code extension, JetBrains plugin, and built-in LSP server</li>
          </ul>
        </section>

        {/* ─── INSTALLATION ─── */}
        <section id="installation" className={styles.section}>
          <h2 className={styles.sectionTitle}>Installation</h2>
          <p className={styles.sectionDesc}>
            Veld is available on all major package managers. Pick whichever works best for your setup.
          </p>

          <div className={styles.installTabs}>
            {Object.keys(installCommands).map((key) => (
              <button
                key={key}
                className={`${styles.installTab} ${installTab === key ? styles.installTabActive : ''}`}
                onClick={() => setInstallTab(key)}
              >
                {key === 'binary' ? 'Binary' : key}
              </button>
            ))}
          </div>

          <CodeBlock title="Terminal">
            {`$ ${installCommands[installTab].install}\n\n# Verify installation:\n$ veld --version\nveld v0.1.0\n\n# Run generation:\n$ ${installCommands[installTab].run}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>System Requirements</h3>
          <ul className={styles.featureList}>
            <li><strong>OS:</strong> Windows, macOS, or Linux (amd64 & arm64)</li>
            <li><strong>Runtime:</strong> None required &mdash; Veld is a standalone binary compiled from Go</li>
            <li><strong>Node.js:</strong> Required only if using <code>npx</code> to run Veld or for Node.js backend output</li>
            <li><strong>Python:</strong> Required only for Python backend output</li>
          </ul>

          <h3 className={styles.sectionSubtitle}>Verify Installation</h3>
          <CodeBlock title="Terminal">
            {`$ veld --version\nveld v0.1.0\n\n$ veld --help\nVeld — Contract-first API code generator\n\nUsage:\n  veld [command]\n\nAvailable Commands:\n  init        Initialize a new Veld project\n  generate    Generate backend and frontend code\n  validate    Validate contract files\n  watch       Watch for changes and auto-regenerate\n  openapi     Export OpenAPI 3.0 spec\n  schema      Generate database schemas\n  docs        Generate API documentation\n  diff        Show contract differences\n  ast         Dump AST as JSON\n  clean       Remove generated output\n  lsp         Start LSP server\n  help        Help about any command`}
          </CodeBlock>
        </section>

        {/* ─── QUICK START ─── */}
        <section id="quickstart" className={styles.section}>
          <h2 className={styles.sectionTitle}>Quick Start</h2>
          <p className={styles.sectionDesc}>
            Get up and running with Veld in under 2 minutes.
          </p>

          <h3 className={styles.sectionSubtitle}>Step 1: Initialize a new project</h3>
          <CodeBlock title="Terminal">
            {`$ mkdir my-api && cd my-api\n$ veld init\n\n✓ Created veld/veld.config.json\n✓ Created veld/app.veld\n✓ Created veld/models/\n✓ Created veld/modules/\n\nDone! Edit veld/app.veld to define your API contract.`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Step 2: Write your contract</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`model User {
  id:    uuid
  email: string
  name:  string
  role:  Role   @default(user)
}

model CreateUserInput {
  email: string
  name:  string
}

enum Role { admin user guest }

module Users {
  prefix: /api/v1

  action ListUsers {
    method: GET
    path:   /users
    output: User[]
  }

  action GetUser {
    method: GET
    path:   /users/:id
    output: User
  }

  action CreateUser {
    method: POST
    path:   /users
    input:  CreateUserInput
    output: User
  }

  action DeleteUser {
    method: DELETE
    path:   /users/:id
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Step 3: Generate code</h3>
          <CodeBlock title="Terminal">
            {`$ veld generate\n\n✓ Generated types/users.ts\n✓ Generated interfaces/IUsersService.ts\n✓ Generated routes/users.routes.ts\n✓ Generated schemas/schemas.ts\n✓ Generated client/api.ts\n✓ Generated index.ts\n✓ Generated package.json\n\nDone! 7 files generated in generated/`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Step 4: Implement your service</h3>
          <CodeBlock title="src/services/UsersService.ts" lang="ts">
            {`import { IUsersService } from '@veld/generated/interfaces/IUsersService';
import { User, CreateUserInput } from '@veld/generated/types';

export class UsersService implements IUsersService {
  async listUsers(): Promise<User[]> {
    return db.users.findMany();
  }

  async getUser(id: string): Promise<User> {
    return db.users.findUnique({ where: { id } });
  }

  async createUser(input: CreateUserInput): Promise<User> {
    return db.users.create({ data: input });
  }

  async deleteUser(id: string): Promise<void> {
    await db.users.delete({ where: { id } });
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Step 5: Wire it up</h3>
          <CodeBlock title="src/index.ts" lang="ts">
            {`import express from 'express';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = express();
app.use(express.json());

const usersService = new UsersService();
registerUsersRoutes(app, usersService);

app.listen(3000, () => {
  console.log('Server running on http://localhost:3000');
});`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Step 6: Use the frontend SDK</h3>
          <CodeBlock title="frontend/src/api.ts" lang="ts">
            {`import { Users } from '@veld/generated/client/api';

// Fully typed — autocomplete for methods, params, and return types
const users = await Users.listUsers();
// ^? Promise<User[]>

const user = await Users.getUser('user-123');
// ^? Promise<User>

await Users.createUser({
  email: 'alice@example.com',
  name: 'Alice',
});
// ^? Promise<User>`}
          </CodeBlock>
        </section>

        {/* ─── PROJECT STRUCTURE ─── */}
        <section id="project-structure" className={styles.section}>
          <h2 className={styles.sectionTitle}>Project Structure</h2>
          <p className={styles.sectionDesc}>
            After running <code>veld init</code>, your project has this structure:
          </p>
          <div className={styles.tree}>
{`my-project/
├── veld/                        <- all veld source files
│   ├── veld.config.json         <- configuration
│   ├── app.veld                 <- entry point contract
│   ├── models/                  <- model definitions
│   └── modules/                 <- module/action definitions
└── generated/                   <- auto-generated on first \`veld generate\`
    ├── index.ts                 <- barrel export
    ├── package.json             <- @veld/generated alias
    ├── types/                   <- TypeScript interfaces
    ├── interfaces/              <- service contracts
    ├── routes/                  <- route handlers
    ├── schemas/                 <- validation schemas
    └── client/                  <- frontend SDK`}
          </div>
          <div className={styles.infoCard}>
            <strong>Note:</strong> Veld never writes outside the <code>--out</code> directory
            (defaults to <code>generated/</code>). Your source code is never touched. The generated
            directory is safe to delete and regenerate at any time.
          </div>
        </section>

        {/* ─── MODELS ─── */}
        <section id="models" className={styles.section}>
          <h2 className={styles.sectionTitle}>Models</h2>
          <p className={styles.sectionDesc}>
            Models define data structures in your API. Each model becomes a TypeScript interface,
            a Zod schema, and corresponding types in your chosen backend language.
          </p>
          <CodeBlock title="models/user.veld" lang="veld">
            {`model User {
  description: "A registered user in the system"
  id:        uuid
  email:     string
  name:      string
  age?:      int              // optional field
  tags:      string[]         // array type
  metadata:  Map<string, string>  // map/record type
  role:      Role  @default(user) // default value
  createdAt: datetime
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Model Syntax</h3>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Syntax</th>
                <th>Meaning</th>
                <th>Example</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><code>fieldName: type</code></td>
                <td>Required field</td>
                <td><code>email: string</code></td>
              </tr>
              <tr>
                <td><code>fieldName?: type</code></td>
                <td>Optional field</td>
                <td><code>bio?: string</code></td>
              </tr>
              <tr>
                <td><code>field: type[]</code></td>
                <td>Array of values</td>
                <td><code>tags: string[]</code></td>
              </tr>
              <tr>
                <td><code>{'field: Map<K, V>'}</code></td>
                <td>Key-value map</td>
                <td><code>{'metadata: Map<string, string>'}</code></td>
              </tr>
              <tr>
                <td><code>@default(value)</code></td>
                <td>Default value decorator</td>
                <td><code>role: Role @default(user)</code></td>
              </tr>
              <tr>
                <td><code>description: "..."</code></td>
                <td>Model description (for docs/OpenAPI)</td>
                <td><code>description: "A user account"</code></td>
              </tr>
            </tbody>
          </table>
        </section>

        {/* ─── ENUMS ─── */}
        <section id="enums" className={styles.section}>
          <h2 className={styles.sectionTitle}>Enums</h2>
          <p className={styles.sectionDesc}>
            Enums define a set of named constants. They generate TypeScript union types,
            Zod enums, and Python string enums.
          </p>
          <CodeBlock title="Example" lang="veld">
            {`// Single-line enum
enum Role { admin user guest }

// Multi-line enum
enum OrderStatus {
  pending
  confirmed
  shipped
  delivered
  cancelled
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Generated Output</h3>
          <CodeBlock title="TypeScript" lang="ts">
            {`export type Role = 'admin' | 'user' | 'guest';

export type OrderStatus = 'pending' | 'confirmed' | 'shipped' | 'delivered' | 'cancelled';`}
          </CodeBlock>

          <CodeBlock title="Zod Schema" lang="ts">
            {`export const RoleSchema = z.enum(['admin', 'user', 'guest']);

export const OrderStatusSchema = z.enum(['pending', 'confirmed', 'shipped', 'delivered', 'cancelled']);`}
          </CodeBlock>
        </section>

        {/* ─── MODULES & ACTIONS ─── */}
        <section id="modules" className={styles.section}>
          <h2 className={styles.sectionTitle}>Modules & Actions</h2>
          <p className={styles.sectionDesc}>
            Modules group related API endpoints. Each module has a prefix and contains actions
            that define individual HTTP endpoints.
          </p>
          <CodeBlock title="modules/users.veld" lang="veld">
            {`module Users {
  description: "User management endpoints"
  prefix: /api/v1

  action ListUsers {
    description: "List all users with pagination"
    method: GET
    path:   /users
    query:  ListUsersQuery
    output: User[]
  }

  action GetUser {
    description: "Get a user by ID"
    method: GET
    path:   /users/:id
    output: User
  }

  action CreateUser {
    description: "Create a new user"
    method: POST
    path:   /users
    input:  CreateUserInput
    output: User
    middleware: AuthGuard
  }

  action UpdateUser {
    method: PUT
    path:   /users/:id
    input:  UpdateUserInput
    output: User
  }

  action DeleteUser {
    method: DELETE
    path:   /users/:id
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Action Fields</h3>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Field</th>
                <th>Required</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><code>method</code></td>
                <td>Yes</td>
                <td>HTTP method: <code>GET</code>, <code>POST</code>, <code>PUT</code>, <code>DELETE</code>, <code>PATCH</code>, or <code>WS</code></td>
              </tr>
              <tr>
                <td><code>path</code></td>
                <td>Yes</td>
                <td>URL path, supports params like <code>/users/:id</code></td>
              </tr>
              <tr>
                <td><code>input</code></td>
                <td>No</td>
                <td>Request body model name. Generates Zod validation.</td>
              </tr>
              <tr>
                <td><code>output</code></td>
                <td>No</td>
                <td>Response body model/type. <code>User[]</code> for arrays.</td>
              </tr>
              <tr>
                <td><code>query</code></td>
                <td>No</td>
                <td>Query parameters model name</td>
              </tr>
              <tr>
                <td><code>errors</code></td>
                <td>No</td>
                <td>Error codes: <code>[NotFound, Forbidden]</code></td>
              </tr>
              <tr>
                <td><code>middleware</code></td>
                <td>No</td>
                <td>Middleware name (e.g. <code>AuthGuard</code>)</td>
              </tr>
              <tr>
                <td><code>description</code></td>
                <td>No</td>
                <td>Action description (used in OpenAPI/docs)</td>
              </tr>
              <tr>
                <td><code>stream</code></td>
                <td>No</td>
                <td>WebSocket message type (requires <code>method: WS</code>)</td>
              </tr>
            </tbody>
          </table>

          <h3 className={styles.sectionSubtitle}>HTTP Status Codes</h3>
          <p className={styles.sectionDesc}>
            Veld automatically generates the correct HTTP status codes:
          </p>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Method</th>
                <th>With Output</th>
                <th>Without Output</th>
              </tr>
            </thead>
            <tbody>
              <tr><td><code>GET</code></td><td>200 OK</td><td>200 OK</td></tr>
              <tr><td><code>POST</code></td><td>201 Created</td><td>201 Created</td></tr>
              <tr><td><code>PUT</code></td><td>200 OK</td><td>200 OK</td></tr>
              <tr><td><code>PATCH</code></td><td>200 OK</td><td>200 OK</td></tr>
              <tr><td><code>DELETE</code></td><td>200 OK</td><td>204 No Content</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── FIELD TYPES ─── */}
        <section id="types" className={styles.section}>
          <h2 className={styles.sectionTitle}>Field Types</h2>
          <p className={styles.sectionDesc}>
            Veld supports a set of built-in primitive types that map to the appropriate types
            in each target language and validation library.
          </p>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Veld</th>
                <th>TypeScript</th>
                <th>Python</th>
                <th>Zod</th>
                <th>Pydantic</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><code>string</code></td><td><code>string</code></td><td><code>str</code></td>
                <td><code>z.string()</code></td><td><code>str</code></td>
              </tr>
              <tr>
                <td><code>int</code></td><td><code>number</code></td><td><code>int</code></td>
                <td><code>z.number().int()</code></td><td><code>int</code></td>
              </tr>
              <tr>
                <td><code>float</code></td><td><code>number</code></td><td><code>float</code></td>
                <td><code>z.number()</code></td><td><code>float</code></td>
              </tr>
              <tr>
                <td><code>bool</code></td><td><code>boolean</code></td><td><code>bool</code></td>
                <td><code>z.boolean()</code></td><td><code>bool</code></td>
              </tr>
              <tr>
                <td><code>date</code></td><td><code>string</code></td><td><code>str</code></td>
                <td><code>z.string().date()</code></td><td><code>str</code></td>
              </tr>
              <tr>
                <td><code>datetime</code></td><td><code>string</code></td><td><code>str</code></td>
                <td><code>z.string().datetime()</code></td><td><code>str</code></td>
              </tr>
              <tr>
                <td><code>uuid</code></td><td><code>string</code></td><td><code>str</code></td>
                <td><code>z.string().uuid()</code></td><td><code>str</code></td>
              </tr>
              <tr>
                <td><code>T[]</code></td><td><code>T[]</code></td><td><code>List[T]</code></td>
                <td><code>z.array(TSchema)</code></td><td><code>List[T]</code></td>
              </tr>
              <tr>
                <td><code>{'Map<string, V>'}</code></td><td><code>{'Record<string, V>'}</code></td><td><code>{'Dict[str, V]'}</code></td>
                <td><code>z.record(z.string(), V)</code></td><td><code>{'Dict[str, V]'}</code></td>
              </tr>
            </tbody>
          </table>
          <div className={styles.infoCard}>
            <strong>Custom types:</strong> Any <code>PascalCase</code> name not in the built-in
            types is treated as a reference to a model or enum defined elsewhere in your contract.
          </div>
        </section>

        {/* ─── INHERITANCE ─── */}
        <section id="inheritance" className={styles.section}>
          <h2 className={styles.sectionTitle}>Inheritance (extends)</h2>
          <p className={styles.sectionDesc}>
            Models can extend other models to inherit all their fields. This generates
            TypeScript <code>interface X extends Y</code>, Zod <code>.extend()</code>,
            and Python class inheritance.
          </p>
          <CodeBlock title="Example" lang="veld">
            {`model BaseEntity {
  id:        uuid
  createdAt: datetime
  updatedAt: datetime
}

model User extends BaseEntity {
  email: string
  name:  string
  role:  Role
}

model Admin extends User {
  permissions: string[]
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Generated TypeScript</h3>
          <CodeBlock title="types/users.ts" lang="ts">
            {`export interface BaseEntity {
  id: string;
  createdAt: string;
  updatedAt: string;
}

export interface User extends BaseEntity {
  email: string;
  name: string;
  role: Role;
}

export interface Admin extends User {
  permissions: string[];
}`}
          </CodeBlock>
          <div className={styles.warningCard}>
            <strong>Circular inheritance</strong> is detected and rejected by the validator.
            For example, <code>A extends B</code> and <code>B extends A</code> will produce
            a clear error with file and line numbers.
          </div>
        </section>

        {/* ─── MAPS ─── */}
        <section id="maps" className={styles.section}>
          <h2 className={styles.sectionTitle}>Maps</h2>
          <p className={styles.sectionDesc}>
            Use <code>{'Map<K, V>'}</code> syntax to define key-value pair fields. Maps generate
            <code> {'Record<string, V>'}</code> in TypeScript and <code>{'Dict[str, V]'}</code> in Python.
          </p>
          <CodeBlock title="Example" lang="veld">
            {`model Config {
  settings:   Map<string, string>
  features:   Map<string, bool>
  metadata:   Map<string, int>
}`}
          </CodeBlock>
          <div className={styles.infoCard}>
            <strong>Note:</strong> Map keys are always <code>string</code>. The value type can be
            any built-in type or a reference to a model/enum.
          </div>
        </section>

        {/* ─── IMPORTS ─── */}
        <section id="imports" className={styles.section}>
          <h2 className={styles.sectionTitle}>Import System</h2>
          <p className={styles.sectionDesc}>
            Veld supports two import styles for organizing contracts across multiple files.
          </p>

          <h3 className={styles.sectionSubtitle}>Alias-based imports (recommended)</h3>
          <CodeBlock title="app.veld" lang="veld">
            {`import @models/user
import @models/product
import @modules/users
import @modules/shop`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            Aliases are resolved from the project root using the <code>aliases</code> config.
            Default aliases include: <code>@models</code>, <code>@modules</code>, <code>@types</code>,
            <code> @enums</code>, <code>@schemas</code>, <code>@services</code>, <code>@lib</code>,
            <code> @common</code>, <code>@shared</code>.
          </p>

          <h3 className={styles.sectionSubtitle}>Relative imports (legacy)</h3>
          <CodeBlock title="app.veld" lang="veld">
            {`import "./models/user.veld"
import "./modules/users.veld"`}
          </CodeBlock>

          <div className={styles.infoCard}>
            <strong>Both styles</strong> are fully supported in the CLI, VS Code extension, and
            JetBrains plugin. Alias imports are preferred for cleaner, more portable contracts.
          </div>
        </section>

        {/* ─── WEBSOCKETS ─── */}
        <section id="websockets" className={styles.section}>
          <h2 className={styles.sectionTitle}>WebSockets</h2>
          <p className={styles.sectionDesc}>
            Veld supports WebSocket actions with the <code>WS</code> method and
            <code> stream</code> field for typed message payloads.
          </p>
          <CodeBlock title="Example" lang="veld">
            {`model ChatMessage {
  userId:  uuid
  content: string
  sentAt:  datetime
}

module Chat {
  prefix: /ws

  action ChatStream {
    method: WS
    path:   /chat/:roomId
    stream: ChatMessage
  }
}`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            WebSocket actions generate typed connect methods in the frontend SDK and comment stubs
            in the backend route handlers. The <code>stream</code> field specifies the message type
            that flows through the WebSocket connection.
          </p>
          <div className={styles.warningCard}>
            <strong>Validation:</strong> The <code>stream</code> field is only valid on
            <code> method: WS</code> actions. WS actions require a <code>stream</code> type.
            Using <code>input</code>/<code>output</code> on WS actions is not allowed.
          </div>
        </section>

        {/* ─── CLI OVERVIEW ─── */}
        <section id="cli-overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>CLI Overview</h2>
          <p className={styles.sectionDesc}>
            The Veld CLI is a single binary with subcommands for every stage of the workflow.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld --help

Usage:
  veld [command]

Available Commands:
  init        Initialize a new Veld project
  generate    Generate backend and frontend code
  validate    Validate contract files
  watch       Watch for changes and auto-regenerate
  openapi     Export OpenAPI 3.0 spec
  schema      Generate database schemas (Prisma/SQL)
  docs        Generate API documentation
  diff        Show contract differences
  ast         Dump AST as JSON
  clean       Remove generated output directory
  lsp         Start LSP server
  help        Help about any command

Flags:
  -h, --help      help for veld
  -v, --version   version for veld`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld init ─── */}
        <section id="cli-init" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld init</h2>
          <p className={styles.sectionDesc}>
            Scaffolds a new Veld project in the current directory. Creates the <code>veld/</code> folder
            with a config file, entry point, and subdirectories for models and modules.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld init

✓ Created veld/veld.config.json
✓ Created veld/app.veld
✓ Created veld/models/
✓ Created veld/modules/`}
          </CodeBlock>
          <div className={styles.warningCard}>
            <strong>Safety:</strong> <code>veld init</code> exits with code 1 if the <code>veld/</code>
            directory already exists &mdash; it will never overwrite existing files.
          </div>
        </section>

        {/* ─── CLI: veld generate ─── */}
        <section id="cli-generate" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld generate</h2>
          <p className={styles.sectionDesc}>
            The main command. Reads your contract, validates it, and generates all output files.
          </p>
          <CodeBlock title="Terminal">
            {`# Use config auto-detection (reads veld.config.json):
$ veld generate

# Specify all options explicitly:
$ veld generate \\
  --backend=node \\
  --frontend=typescript \\
  --input=veld/app.veld \\
  --out=./generated

# Preview without writing files:
$ veld generate --dry-run`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Flags</h3>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Flag</th>
                <th>Default</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              <tr><td><code>--backend</code></td><td>from config</td><td>Backend emitter: <code>node</code>, <code>python</code>, <code>go</code>, <code>java</code>, <code>csharp</code>, <code>php</code>, <code>rust</code></td></tr>
              <tr><td><code>--frontend</code></td><td>from config</td><td>Frontend emitter: <code>typescript</code>, <code>dart</code>, <code>kotlin</code>, <code>swift</code>, <code>none</code></td></tr>
              <tr><td><code>--input</code></td><td>from config</td><td>Entry <code>.veld</code> file path</td></tr>
              <tr><td><code>--out</code></td><td>from config</td><td>Output directory</td></tr>
              <tr><td><code>--dry-run</code></td><td><code>false</code></td><td>Preview generated files without writing</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── CLI: veld validate ─── */}
        <section id="cli-validate" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld validate</h2>
          <p className={styles.sectionDesc}>
            Validates your contract without generating any output. Reports errors with
            file names, line numbers, and source code snippets.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld validate

✓ Contract is valid (3 models, 1 enum, 2 modules, 5 actions)

# Example error output:
$ veld validate

✗ Validation failed:

  veld/models/user.veld:5
    role: FooBar @default(admin)
          ^^^^^^
  Error: Unknown type "FooBar" — did you mean "Role"?

  veld/modules/users.veld:12
    input: NonExistent
           ^^^^^^^^^^^
  Error: Input type "NonExistent" is not defined`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld watch ─── */}
        <section id="cli-watch" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld watch</h2>
          <p className={styles.sectionDesc}>
            Watches your <code>.veld</code> files for changes and auto-regenerates with a
            500ms debounce. Perfect for development.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld watch

Watching for changes in veld/ ...

[12:00:01] Changed: veld/models/user.veld
[12:00:01] Regenerating...
[12:00:01] ✓ Done (7 files in 42ms)`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld openapi ─── */}
        <section id="cli-openapi" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld openapi</h2>
          <p className={styles.sectionDesc}>
            Exports an OpenAPI 3.0 specification from your contract. Output to stdout or a file.
          </p>
          <CodeBlock title="Terminal">
            {`# Print to stdout:
$ veld openapi

# Write to file:
$ veld openapi -o openapi.json

# Pipe to another tool:
$ veld openapi | jq '.paths'`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld schema ─── */}
        <section id="cli-schema" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld schema</h2>
          <p className={styles.sectionDesc}>
            Generates database schemas from your models. Supports Prisma schema format and raw SQL DDL.
          </p>
          <CodeBlock title="Terminal">
            {`# Generate Prisma schema:
$ veld schema --format=prisma -o schema.prisma

# Generate SQL DDL:
$ veld schema --format=sql -o schema.sql`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld docs ─── */}
        <section id="cli-docs" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld docs</h2>
          <p className={styles.sectionDesc}>
            Generates human-readable API documentation from your contract. Useful for teams
            and stakeholders who don't read code.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld docs -o api-docs.md`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld diff ─── */}
        <section id="cli-diff" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld diff</h2>
          <p className={styles.sectionDesc}>
            Shows the differences between contract versions. Detects added/removed/changed
            models, fields, actions, and types.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld diff --old=v1/app.veld --new=v2/app.veld

+ Added model: PaymentMethod
~ Changed model: User
  + Added field: avatarUrl (string)
  - Removed field: profilePic
~ Changed action: CreateUser
  ~ input changed: CreateUserInput -> CreateUserInputV2`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld ast ─── */}
        <section id="cli-ast" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld ast</h2>
          <p className={styles.sectionDesc}>
            Dumps the parsed AST as JSON. Useful for debugging, tooling, or building custom
            code generators on top of Veld's parser.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld ast | jq '.models[0]'
{
  "name": "User",
  "fields": [
    { "name": "id", "type": "uuid", "optional": false },
    { "name": "email", "type": "string", "optional": false }
  ]
}`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld clean ─── */}
        <section id="cli-clean" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld clean</h2>
          <p className={styles.sectionDesc}>
            Removes the generated output directory. A clean slate for regeneration.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld clean\n\n✓ Removed generated/`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG FILE ─── */}
        <section id="config-file" className={styles.section}>
          <h2 className={styles.sectionTitle}>Configuration File</h2>
          <p className={styles.sectionDesc}>
            Veld uses a JSON configuration file to set defaults for all CLI commands.
            This file is created automatically by <code>veld init</code>.
          </p>
          <CodeBlock title="veld/veld.config.json" lang="json">
            {`{
  "input": "app.veld",
  "backend": "node",
  "frontend": "typescript",
  "out": "../generated",
  "baseUrl": "/api/v1",
  "aliases": {
    "models": "models",
    "modules": "modules",
    "auth": "services/auth"
  }
}`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG FIELDS ─── */}
        <section id="config-fields" className={styles.section}>
          <h2 className={styles.sectionTitle}>Config Fields</h2>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Field</th>
                <th>Type</th>
                <th>Default</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><code>input</code></td><td>string</td><td><em>required</em></td>
                <td>Entry <code>.veld</code> file (relative to config location)</td>
              </tr>
              <tr>
                <td><code>backend</code></td><td>string</td><td><code>"node"</code></td>
                <td>Backend emitter: <code>node</code>, <code>python</code>, <code>go</code>, <code>java</code>, <code>csharp</code>, <code>php</code>, <code>rust</code></td>
              </tr>
              <tr>
                <td><code>frontend</code></td><td>string</td><td><code>"typescript"</code></td>
                <td>Frontend emitter: <code>typescript</code>, <code>react</code> (alias), <code>dart</code>, <code>flutter</code> (alias), <code>kotlin</code>, <code>swift</code>, <code>none</code></td>
              </tr>
              <tr>
                <td><code>out</code></td><td>string</td><td><code>"./generated"</code></td>
                <td>Output directory (relative to config location)</td>
              </tr>
              <tr>
                <td><code>baseUrl</code></td><td>string</td><td><code>""</code></td>
                <td>Baked into frontend SDK. If empty, uses <code>process.env.VELD_API_URL</code></td>
              </tr>
              <tr>
                <td><code>aliases</code></td><td>object</td><td>built-in defaults</td>
                <td>Custom <code>@alias</code> to relative directory mappings</td>
              </tr>
            </tbody>
          </table>
        </section>

        {/* ─── CONFIG ALIASES ─── */}
        <section id="config-aliases" className={styles.section}>
          <h2 className={styles.sectionTitle}>Import Aliases</h2>
          <p className={styles.sectionDesc}>
            Aliases map short <code>@name</code> prefixes to directories, relative to the config file.
          </p>
          <CodeBlock title="veld.config.json" lang="json">
            {`{
  "aliases": {
    "models": "models",
    "modules": "modules",
    "types": "types",
    "enums": "enums",
    "schemas": "schemas",
    "services": "services",
    "lib": "lib",
    "common": "common",
    "shared": "shared"
  }
}`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            With this config, <code>import @models/user</code> resolves to <code>veld/models/user.veld</code>.
            You can add custom aliases for any directory structure:
          </p>
          <CodeBlock title="Custom alias example" lang="json">
            {`{
  "aliases": {
    "auth": "services/auth",
    "payments": "features/payments/contracts"
  }
}`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG DETECTION ─── */}
        <section id="config-detection" className={styles.section}>
          <h2 className={styles.sectionTitle}>Config Auto-Detection</h2>
          <p className={styles.sectionDesc}>
            When you run <code>veld generate</code> (no flags), Veld searches for the config file
            in this order:
          </p>
          <ol className={styles.featureList}>
            <li><code>./veld.config.json</code> in the current directory</li>
            <li><code>./veld/veld.config.json</code> in the <code>veld/</code> subdirectory</li>
          </ol>
          <p className={styles.sectionDesc}>
            CLI flags always override config file values. For example:
          </p>
          <CodeBlock title="Terminal">
            {`# Config says backend=node, but override with python:
$ veld generate --backend=python`}
          </CodeBlock>
        </section>

        {/* ─── OUTPUT OVERVIEW ─── */}
        <section id="output-overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>Generated Output Overview</h2>
          <p className={styles.sectionDesc}>
            Veld generates a complete set of files organized by purpose. All generated files
            begin with <code>// AUTO-GENERATED BY VELD — DO NOT EDIT</code>.
          </p>
          <div className={styles.infoCard}>
            <strong>Key principles:</strong> Output is deterministic (same input = same output),
            Veld never writes outside <code>--out</code>, and generated code has zero runtime
            dependencies for type-only usage.
          </div>
        </section>

        {/* ─── OUTPUT: NODE ─── */}
        <section id="output-node" className={styles.section}>
          <h2 className={styles.sectionTitle}>Node.js Backend Output</h2>
          <div className={styles.tree}>
{`generated/
├── index.ts              # Barrel export for clean imports
├── package.json          # @veld/generated package alias
├── types/
│   ├── users.ts          # Types owned by Users module
│   ├── auth.ts           # Types owned by Auth module + re-exports shared
│   └── index.ts          # Barrel re-export of all module type files
├── interfaces/
│   └── IUsersService.ts  # Service contract interface (typed path params)
├── routes/
│   └── users.routes.ts   # Route handlers: try/catch, Zod validation, HTTP status codes
├── schemas/
│   └── schemas.ts        # Zod validation schemas (supports extends)
└── client/
    └── api.ts            # Frontend SDK with VeldApiError, path params`}
          </div>
          <p className={styles.sectionDesc}>
            Types are emitted into per-module files. Each type is <strong>defined</strong> in exactly
            one file (the first module to use it). Other modules re-export shared types. A
            barrel <code>types/index.ts</code> re-exports everything.
          </p>
        </section>

        {/* ─── OUTPUT: PYTHON ─── */}
        <section id="output-python" className={styles.section}>
          <h2 className={styles.sectionTitle}>Python Backend Output</h2>
          <div className={styles.tree}>
{`generated/
├── __init__.py
├── types/
│   ├── users.py             # Types owned by Users module
│   ├── auth.py              # Types owned by Auth module + re-imports shared
│   └── __init__.py          # Barrel re-import
├── interfaces/
│   └── i_users_service.py   # ABC service contract
├── routes/
│   └── users_routes.py      # Flask handlers: try/except, Pydantic validation
└── schemas/
    └── schemas.py           # Pydantic BaseModel schemas`}
          </div>
        </section>

        {/* ─── OUTPUT: FRONTEND ─── */}
        <section id="output-frontend" className={styles.section}>
          <h2 className={styles.sectionTitle}>Frontend SDK</h2>
          <p className={styles.sectionDesc}>
            The generated frontend SDK uses native <code>fetch</code> (no axios dependency)
            with full type safety.
          </p>
          <h3 className={styles.sectionSubtitle}>Features</h3>
          <ul className={styles.featureList}>
            <li><strong>VeldApiError</strong> class with <code>status</code> and <code>body</code> fields for type-safe error handling</li>
            <li><strong>Path parameter interpolation:</strong> <code>/users/:id</code> becomes <code>{'/users/${id}'}</code> with typed <code>id: string</code> param</li>
            <li><strong>All HTTP methods:</strong> <code>get()</code>, <code>post()</code>, <code>put()</code>, <code>patch()</code>, <code>del()</code></li>
            <li><strong>Base URL:</strong> configurable via config or <code>process.env.VELD_API_URL</code></li>
            <li><strong>Zero dependencies:</strong> uses only native <code>fetch</code></li>
          </ul>
          <CodeBlock title="client/api.ts (generated)" lang="ts">
            {`// AUTO-GENERATED BY VELD — DO NOT EDIT

export class VeldApiError extends Error {
  constructor(public status: number, public body: unknown) {
    super(\`API error \${status}\`);
  }
}

const BASE_URL = process.env.VELD_API_URL || '';

export const Users = {
  async listUsers(): Promise<User[]> {
    const res = await fetch(\`\${BASE_URL}/api/v1/users\`);
    if (!res.ok) throw new VeldApiError(res.status, await res.json());
    return res.json();
  },

  async getUser(id: string): Promise<User> {
    const res = await fetch(\`\${BASE_URL}/api/v1/users/\${id}\`);
    if (!res.ok) throw new VeldApiError(res.status, await res.json());
    return res.json();
  },

  async createUser(data: CreateUserInput): Promise<User> {
    const res = await fetch(\`\${BASE_URL}/api/v1/users\`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!res.ok) throw new VeldApiError(res.status, await res.json());
    return res.json();
  },
};`}
          </CodeBlock>
        </section>

        {/* ─── OUTPUT: SCHEMAS ─── */}
        <section id="output-schemas" className={styles.section}>
          <h2 className={styles.sectionTitle}>Validation Schemas</h2>
          <p className={styles.sectionDesc}>
            Veld generates validation schemas that are used automatically in route handlers.
          </p>
          <h3 className={styles.sectionSubtitle}>Node.js (Zod)</h3>
          <CodeBlock title="schemas/schemas.ts" lang="ts">
            {`import { z } from 'zod';

export const RoleSchema = z.enum(['admin', 'user', 'guest']);

export const UserSchema = z.object({
  id: z.string().uuid(),
  email: z.string(),
  name: z.string(),
  role: RoleSchema.default('user'),
});

export const CreateUserInputSchema = z.object({
  email: z.string(),
  name: z.string(),
});`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Python (Pydantic)</h3>
          <CodeBlock title="schemas/schemas.py">
            {`from pydantic import BaseModel
from typing import Optional

class UserSchema(BaseModel):
    id: str
    email: str
    name: str
    role: str = 'user'

class CreateUserInputSchema(BaseModel):
    email: str
    name: str`}
          </CodeBlock>
        </section>

        {/* ─── OUTPUT: ROUTES ─── */}
        <section id="output-routes" className={styles.section}>
          <h2 className={styles.sectionTitle}>Route Handlers</h2>
          <p className={styles.sectionDesc}>
            Generated route handlers include try/catch wrapping, input validation, correct HTTP
            status codes, and path parameter extraction.
          </p>
          <CodeBlock title="routes/users.routes.ts" lang="ts">
            {`import { IUsersService } from '../interfaces/IUsersService';
import { CreateUserInputSchema } from '../schemas/schemas';

export function registerUsersRoutes(router: any, service: IUsersService) {

  router.get('/api/v1/users', async (req, res) => {
    try {
      const result = await service.listUsers();
      res.status(200).json(result);
    } catch (err) {
      res.status(500).json({ error: 'Internal server error' });
    }
  });

  router.post('/api/v1/users', async (req, res) => {
    try {
      const input = CreateUserInputSchema.parse(req.body);
      const result = await service.createUser(input);
      res.status(201).json(result);
    } catch (err) {
      if (err.name === 'ZodError') {
        return res.status(400).json({ errors: err.issues });
      }
      res.status(500).json({ error: 'Internal server error' });
    }
  });

  router.delete('/api/v1/users/:id', async (req, res) => {
    try {
      await service.deleteUser(req.params.id);
      res.status(204).end();
    } catch (err) {
      res.status(500).json({ error: 'Internal server error' });
    }
  });
}`}
          </CodeBlock>
          <div className={styles.infoCard}>
            <strong>Framework agnostic:</strong> The <code>router</code> parameter accepts
            <code> any</code> &mdash; wire in Express, Fastify, Hono, or any router with
            <code> .get()</code>/<code>.post()</code> methods.
          </div>
        </section>

        {/* ─── USAGE: BACKEND ─── */}
        <section id="usage-backend" className={styles.section}>
          <h2 className={styles.sectionTitle}>Backend Integration</h2>
          <p className={styles.sectionDesc}>
            Implement the generated service interface, then wire it to your router.
            Veld never touches your business logic files.
          </p>

          <h3 className={styles.sectionSubtitle}>1. Implement the interface</h3>
          <CodeBlock title="src/services/UsersService.ts" lang="ts">
            {`import { IUsersService } from '@veld/generated/interfaces/IUsersService';
import { User, CreateUserInput } from '@veld/generated/types';

export class UsersService implements IUsersService {
  async listUsers(): Promise<User[]> {
    // Your business logic here
    return await db.users.findMany();
  }

  async getUser(id: string): Promise<User> {
    const user = await db.users.findUnique({ where: { id } });
    if (!user) throw new Error('User not found');
    return user;
  }

  async createUser(input: CreateUserInput): Promise<User> {
    return await db.users.create({ data: input });
  }

  async deleteUser(id: string): Promise<void> {
    await db.users.delete({ where: { id } });
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>2. Wire routes to your server</h3>
          <CodeBlock title="src/index.ts" lang="ts">
            {`import express from 'express';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = express();
app.use(express.json());

registerUsersRoutes(app, new UsersService());

app.listen(3000);`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Works with any router</h3>
          <CodeBlock title="With Fastify" lang="ts">
            {`import Fastify from 'fastify';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = Fastify();
registerUsersRoutes(app, new UsersService());
app.listen({ port: 3000 });`}
          </CodeBlock>
        </section>

        {/* ─── USAGE: FRONTEND ─── */}
        <section id="usage-frontend" className={styles.section}>
          <h2 className={styles.sectionTitle}>Frontend SDK Usage</h2>
          <p className={styles.sectionDesc}>
            The generated frontend SDK provides fully typed methods for every action in your contract.
          </p>
          <CodeBlock title="Using the SDK" lang="ts">
            {`import { Users } from '@veld/generated/client/api';
import { VeldApiError } from '@veld/generated/client/api';

// List all users
const users = await Users.listUsers();
// ^? User[]

// Get a single user (path param is typed)
const user = await Users.getUser('user-123');
// ^? User

// Create a user (input is typed)
const newUser = await Users.createUser({
  email: 'alice@example.com',
  name: 'Alice',
});
// ^? User

// Error handling
try {
  await Users.getUser('nonexistent');
} catch (err) {
  if (err instanceof VeldApiError) {
    console.error(err.status); // 404
    console.error(err.body);   // { error: '...' }
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Setting the Base URL</h3>
          <CodeBlock title="Environment variable" lang="ts">
            {`// Option 1: Environment variable
// Set VELD_API_URL=https://api.example.com in your .env

// Option 2: Config file — set baseUrl in veld.config.json
// { "baseUrl": "https://api.example.com" }

// The SDK reads from:
// 1. baseUrl from config (baked into generated code)
// 2. process.env.VELD_API_URL (runtime fallback)`}
          </CodeBlock>
        </section>

        {/* ─── USAGE: PATH ALIAS ─── */}
        <section id="usage-path-alias" className={styles.section}>
          <h2 className={styles.sectionTitle}>Path Aliases</h2>
          <p className={styles.sectionDesc}>
            The generated <code>package.json</code> enables the <code>@veld/generated</code> import alias.
            Add this to your <code>tsconfig.json</code> to use it:
          </p>
          <CodeBlock title="tsconfig.json" lang="json">
            {`{
  "compilerOptions": {
    "paths": {
      "@veld/*": ["./generated/*"]
    }
  }
}`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            Then import generated code anywhere:
          </p>
          <CodeBlock title="Usage" lang="ts">
            {`import { User } from '@veld/generated/types';
import { IUsersService } from '@veld/generated/interfaces/IUsersService';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { Users } from '@veld/generated/client/api';`}
          </CodeBlock>
        </section>

        {/* ─── STACKS: BACKENDS ─── */}
        <section id="stacks-backends" className={styles.section}>
          <h2 className={styles.sectionTitle}>Backend Emitters</h2>
          <p className={styles.sectionDesc}>
            Veld supports 7 backend target languages. Each emitter generates types, service interfaces,
            route handlers, and validation schemas in the target language.
          </p>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Flag Value</th>
                <th>Language</th>
                <th>Validation</th>
                <th>Route Style</th>
              </tr>
            </thead>
            <tbody>
              <tr><td><code>node</code></td><td>TypeScript (Node.js)</td><td>Zod</td><td>Express/Fastify/Hono</td></tr>
              <tr><td><code>python</code></td><td>Python</td><td>Pydantic</td><td>Flask</td></tr>
              <tr><td><code>go</code></td><td>Go</td><td>Built-in</td><td>Chi/Mux</td></tr>
              <tr><td><code>java</code></td><td>Java</td><td>Jakarta</td><td>Spring Boot</td></tr>
              <tr><td><code>csharp</code></td><td>C#</td><td>DataAnnotations</td><td>ASP.NET</td></tr>
              <tr><td><code>php</code></td><td>PHP</td><td>Built-in</td><td>Laravel</td></tr>
              <tr><td><code>rust</code></td><td>Rust</td><td>serde</td><td>Actix/Axum</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── STACKS: FRONTENDS ─── */}
        <section id="stacks-frontends" className={styles.section}>
          <h2 className={styles.sectionTitle}>Frontend Emitters</h2>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Flag Value</th>
                <th>Language</th>
                <th>Output File</th>
                <th>Aliases</th>
              </tr>
            </thead>
            <tbody>
              <tr><td><code>typescript</code></td><td>TypeScript</td><td><code>client/api.ts</code></td><td><code>react</code></td></tr>
              <tr><td><code>dart</code></td><td>Dart</td><td><code>client/api_client.dart</code></td><td><code>flutter</code></td></tr>
              <tr><td><code>kotlin</code></td><td>Kotlin</td><td><code>client/ApiClient.kt</code></td><td>&mdash;</td></tr>
              <tr><td><code>swift</code></td><td>Swift</td><td><code>client/APIClient.swift</code></td><td>&mdash;</td></tr>
              <tr><td><code>none</code></td><td>&mdash;</td><td>No frontend SDK generated</td><td>&mdash;</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── STACKS: EXTRAS ─── */}
        <section id="stacks-extras" className={styles.section}>
          <h2 className={styles.sectionTitle}>Extras</h2>
          <p className={styles.sectionDesc}>
            Beyond code generation, Veld can produce additional output formats:
          </p>
          <ul className={styles.featureList}>
            <li><strong>OpenAPI 3.0</strong> &mdash; <code>veld openapi -o openapi.json</code></li>
            <li><strong>Prisma Schema</strong> &mdash; <code>veld schema --format=prisma</code></li>
            <li><strong>SQL DDL</strong> &mdash; <code>veld schema --format=sql</code></li>
            <li><strong>API Documentation</strong> &mdash; <code>veld docs</code></li>
            <li><strong>Contract Diff</strong> &mdash; <code>veld diff</code></li>
          </ul>
        </section>

        {/* ─── EDITOR: VS CODE ─── */}
        <section id="editor-vscode" className={styles.section}>
          <h2 className={styles.sectionTitle}>VS Code Extension</h2>
          <p className={styles.sectionDesc}>
            The Veld VS Code extension provides a first-class editing experience for <code>.veld</code> files.
          </p>
          <h3 className={styles.sectionSubtitle}>Installation</h3>
          <ol className={styles.featureList}>
            <li>Open VS Code</li>
            <li>Go to Extensions (<code>Ctrl+Shift+X</code> / <code>Cmd+Shift+X</code>)</li>
            <li>Search for <strong>"Veld"</strong></li>
            <li>Click Install</li>
          </ol>
          <h3 className={styles.sectionSubtitle}>Features</h3>
          <ul className={styles.featureList}>
            <li>Syntax highlighting for <code>.veld</code> files</li>
            <li>Real-time diagnostics (errors and warnings)</li>
            <li>Autocomplete for keywords, types, and model/enum references</li>
            <li>Hover information for types and actions</li>
            <li>Go-to-definition for model and enum references</li>
            <li>Code snippets for models, modules, and actions</li>
          </ul>
        </section>

        {/* ─── EDITOR: JETBRAINS ─── */}
        <section id="editor-jetbrains" className={styles.section}>
          <h2 className={styles.sectionTitle}>JetBrains Plugin</h2>
          <p className={styles.sectionDesc}>
            Available for IntelliJ IDEA, WebStorm, PyCharm, GoLand, and all JetBrains IDEs.
          </p>
          <h3 className={styles.sectionSubtitle}>Installation</h3>
          <ol className={styles.featureList}>
            <li>Open Settings / Preferences</li>
            <li>Go to Plugins &rarr; Marketplace</li>
            <li>Search for <strong>"Veld"</strong></li>
            <li>Click Install and restart the IDE</li>
          </ol>
          <h3 className={styles.sectionSubtitle}>Features</h3>
          <ul className={styles.featureList}>
            <li>Syntax highlighting and code folding</li>
            <li>Error highlighting and quick-fixes</li>
            <li>Autocomplete for all Veld keywords and types</li>
            <li>Navigate to definition</li>
          </ul>
        </section>

        {/* ─── EDITOR: LSP ─── */}
        <section id="editor-lsp" className={styles.section}>
          <h2 className={styles.sectionTitle}>LSP Server</h2>
          <p className={styles.sectionDesc}>
            Veld includes a built-in Language Server Protocol (LSP) server that any editor
            can use for diagnostics, completions, and hover information.
          </p>
          <CodeBlock title="Start the LSP server">
            {`$ veld lsp

# The LSP server communicates via JSON-RPC 2.0 over stdin/stdout.
# Configure your editor to launch "veld lsp" as the language server
# for .veld files.`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Supported LSP Features</h3>
          <ul className={styles.featureList}>
            <li><strong>textDocument/publishDiagnostics</strong> &mdash; real-time error reporting</li>
            <li><strong>textDocument/completion</strong> &mdash; keyword, type, and reference completions</li>
            <li><strong>textDocument/hover</strong> &mdash; type information on hover</li>
            <li><strong>textDocument/definition</strong> &mdash; go-to-definition for model/enum references</li>
          </ul>
          <h3 className={styles.sectionSubtitle}>Neovim Setup</h3>
          <CodeBlock title="init.lua">
            {`vim.api.nvim_create_autocmd('FileType', {
  pattern = 'veld',
  callback = function()
    vim.lsp.start({
      name = 'veld',
      cmd = { 'veld', 'lsp' },
      root_dir = vim.fs.dirname(
        vim.fs.find({ 'veld.config.json' }, { upward = true })[1]
      ),
    })
  end,
})`}
          </CodeBlock>
        </section>

        {/* ── Cloud Registry ─────────────────────────────────────────── */}

        <section id="registry-overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>Cloud Registry</h2>
          <p className={styles.sectionDesc}>
            The Veld Registry is a typed contract package registry — think npm, but for <code>.veld</code> files.
            Teams publish their contracts once; every repo that consumes them pulls exact versions.
            Generated SDKs are always in sync across services with zero manual copy-paste.
          </p>
          <p className={styles.sectionDesc}>
            The registry is <strong>fully self-hostable</strong> — run it on your own server with a single binary
            and a PostgreSQL database. Or connect to a hosted instance. Either way the CLI workflow is identical.
          </p>
          <h3 className={styles.sectionSubtitle}>How it works</h3>
          <CodeBlock title="Team workflow">{`# Team A publishes their auth contracts
cd auth-service/veld
veld push                          # → @acme/auth@1.2.0 on registry

# Team B consumes them in a completely separate repo
veld pull @acme/auth@1.2.0        # → veld/packages/@acme/auth/
veld generate                      # → fully typed SDK from pulled contracts`}
          </CodeBlock>
        </section>

        <section id="registry-selfhost" className={styles.section}>
          <h2 className={styles.sectionTitle}>Self-Hosting</h2>
          <p className={styles.sectionDesc}>
            The registry server is a single Go binary (<code>veld-registry</code>) that needs a PostgreSQL
            database and a directory for tarball storage. The schema is applied automatically on first start.
          </p>
          <h3 className={styles.sectionSubtitle}>1. Build the server</h3>
          <CodeBlock title="Build">{`go build -o veld-registry ./cmd/registry`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>2. Create a config file</h3>
          <CodeBlock title="registry.config.json" lang="json">{`{
  "addr":    ":8080",
  "dsn":     "postgres://veld:secret@localhost:5432/veld?sslmode=disable",
  "storage": "./packages",
  "secret":  "run: openssl rand -hex 32"
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>3. Create the database and start</h3>
          <CodeBlock title="Terminal">{`createdb veld
./veld-registry --config registry.config.json

# Veld Registry  →  http://localhost:8080
# Web UI available at http://localhost:8080/`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Config priority</h3>
          <p className={styles.sectionDesc}>
            Settings are merged in this order (highest wins):
          </p>
          <CodeBlock>{`CLI flag  >  env var  >  registry.config.json  >  default

# Example: keep secrets out of the file, inject at runtime
VELD_SECRET=mysecret ./veld-registry   # reads rest from registry.config.json`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>All config fields</h3>
          <CodeBlock title="registry.config.json" lang="json">{`{
  "addr":    ":8080",         // listen address  (env: VELD_ADDR)
  "dsn":     "postgres://…",  // PostgreSQL DSN  (env: VELD_DSN)
  "storage": "./packages",    // tarball dir     (env: VELD_STORAGE)
  "secret":  "…"              // JWT secret ≥16c (env: VELD_SECRET)
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Auto-detection</h3>
          <p className={styles.sectionDesc}>
            If <code>--config</code> is omitted, the server looks for <code>registry.config.json</code> then{' '}
            <code>veld/registry.config.json</code> in the current directory.
          </p>
        </section>

        <section id="registry-login" className={styles.section}>
          <h2 className={styles.sectionTitle}>Login &amp; Auth</h2>
          <p className={styles.sectionDesc}>
            Authenticate the CLI against a registry with <code>veld login</code>. Credentials are stored
            in <code>~/.veld/credentials.json</code> with 0600 permissions. Multiple registries can be
            configured simultaneously.
          </p>
          <h3 className={styles.sectionSubtitle}>Interactive login</h3>
          <CodeBlock title="Terminal">{`veld login --registry http://localhost:8080
# Email: you@example.com
# Password: ••••••••
# ✓ Logged in to http://localhost:8080 as yourname`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Token login (CI/CD)</h3>
          <CodeBlock title="Terminal">{`# Create a token in the web UI, then:
veld login --registry http://localhost:8080 --token vtk_xxxxxxxxxxxxxxxx
# ✓ Logged in to http://localhost:8080 as yourname`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Check current identity</h3>
          <CodeBlock title="Terminal">{`veld registry info
# Registry: http://localhost:8080
# User:     yourname (you@example.com)

veld registry list     # show all configured registries`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Logout</h3>
          <CodeBlock title="Terminal">{`veld logout
veld logout --registry http://other-registry.com`}
          </CodeBlock>
        </section>

        <section id="registry-push" className={styles.section}>
          <h2 className={styles.sectionTitle}>Publishing Contracts</h2>
          <p className={styles.sectionDesc}>
            <code>veld push</code> packs your <code>.veld</code> files and <code>veld.config.json</code> into
            a signed tarball and uploads it to the registry. You must be an <strong>admin</strong> or{' '}
            <strong>owner</strong> of the organisation to publish.
          </p>
          <h3 className={styles.sectionSubtitle}>Configure publishing in veld.config.json</h3>
          <CodeBlock title="veld/veld.config.json" lang="json">{`{
  "input":    "app.veld",
  "backend":  "node",
  "frontend": "react",
  "out":      "../generated",

  "registry": {
    "enabled": true,
    "url":     "http://localhost:8080",
    "org":     "acme",
    "package": "auth-service",
    "version": "1.2.0"
  }
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Push</h3>
          <CodeBlock title="Terminal">{`# Reads org/package/version/url from veld.config.json
veld push

# Or override on the CLI
veld push --org acme --name auth-service --version 1.2.0
veld push --registry http://localhost:8080 --org acme --name auth --version 2.0.0

# Output:
# ⬡  Packing contracts from ./veld…
# ⬡  Publishing @acme/auth-service@1.2.0 (4.2 kB)…
# ✓  Published @acme/auth-service@1.2.0`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>What gets packed</h3>
          <ul className={styles.featureList}>
            <li>All <code>.veld</code> files found recursively in the config directory</li>
            <li><code>veld.config.json</code></li>
            <li>Everything is gzip-compressed and SHA-256 signed for integrity verification</li>
          </ul>
        </section>

        <section id="registry-pull" className={styles.section}>
          <h2 className={styles.sectionTitle}>Installing Contracts</h2>
          <p className={styles.sectionDesc}>
            <code>veld pull</code> downloads a versioned contract package, verifies its SHA-256 checksum,
            and extracts it to <code>veld/packages/@org/name/</code>. Pulled contracts are imported
            exactly like local files.
          </p>
          <h3 className={styles.sectionSubtitle}>Pull a package</h3>
          <CodeBlock title="Terminal">{`veld pull @acme/auth-service          # latest version
veld pull @acme/auth-service@1.2.0   # exact version
veld pull @acme/auth-service@1.2.0 --out veld/packages  # custom dir`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Declare dependencies in veld.config.json</h3>
          <CodeBlock title="veld/veld.config.json" lang="json">{`{
  "input": "app.veld",
  "backend": "node",
  "frontend": "react",
  "out": "../generated",

  "registry": {
    "enabled": true,
    "url": "http://localhost:8080"
  },

  "dependencies": {
    "@acme/auth-service": "1.2.0",
    "@acme/shared-types": "2.0.0"
  }
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Use pulled contracts in imports</h3>
          <CodeBlock title="veld/app.veld" lang="veld">{`// Pulled packages are available as @org/package imports
import @acme/auth-service/UserModel
import @acme/shared-types/PaginationMeta

module Orders {
  prefix: /api/orders

  action GetOrders {
    method: GET
    path:   /
    output: PaginationMeta   // ← from pulled package
  }
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>List available versions</h3>
          <CodeBlock title="Terminal">{`veld registry versions @acme/auth-service
# @acme/auth-service — 3 version(s):
#   v1.2.0
#   v1.1.0
#   v1.0.0  [deprecated: use 1.2.0]`}
          </CodeBlock>
        </section>

        <section id="registry-teams" className={styles.section}>
          <h2 className={styles.sectionTitle}>Teams &amp; Organisations</h2>
          <p className={styles.sectionDesc}>
            Packages are published under <em>organisations</em> (the <code>@scope</code>).
            Each org has members with roles that control who can publish, manage members, and delete packages.
          </p>
          <h3 className={styles.sectionSubtitle}>Create an organisation</h3>
          <CodeBlock title="Terminal">{`# Via web UI: http://localhost:8080/#/orgs → New Organisation

# Or use the API directly:
curl -X POST http://localhost:8080/api/v1/orgs \\
  -H "Authorization: Bearer vtk_…" \\
  -H "Content-Type: application/json" \\
  -d '{"name":"acme","display_name":"ACME Corp"}'`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Role permissions</h3>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Action</th>
                <th>Member</th>
                <th>Admin</th>
                <th>Owner</th>
              </tr>
            </thead>
            <tbody>
              <tr><td>Pull private packages</td><td>✓</td><td>✓</td><td>✓</td></tr>
              <tr><td>Publish new versions</td><td>✗</td><td>✓</td><td>✓</td></tr>
              <tr><td>Deprecate versions</td><td>✗</td><td>✓</td><td>✓</td></tr>
              <tr><td>Manage members</td><td>✗</td><td>✓</td><td>✓</td></tr>
              <tr><td>Unpublish versions</td><td>✗</td><td>✗</td><td>✓</td></tr>
            </tbody>
          </table>
          <h3 className={styles.sectionSubtitle}>Add a team member</h3>
          <CodeBlock title="Terminal">{`# Via web UI: http://localhost:8080/#/orgs/acme → Add Member

# REST API:
curl -X POST http://localhost:8080/api/v1/orgs/acme/members \\
  -H "Authorization: Bearer vtk_…" \\
  -d '{"username":"alice","role":"admin"}'`}
          </CodeBlock>
        </section>

        <section id="registry-tokens" className={styles.section}>
          <h2 className={styles.sectionTitle}>API Tokens</h2>
          <p className={styles.sectionDesc}>
            API tokens are prefixed with <code>vtk_</code> and stored as SHA-256 hashes — the plain token
            is shown <strong>only once</strong> at creation time. Use tokens for CI/CD pipelines and
            non-interactive CLI auth.
          </p>
          <h3 className={styles.sectionSubtitle}>Create a token</h3>
          <CodeBlock title="Terminal">{`# Via web UI: http://localhost:8080/#/tokens → New Token

# Via CLI (after logging in):
veld registry token create --name ci-deploy --scopes read,write`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Token scopes</h3>
          <ul className={styles.featureList}>
            <li><code>read</code> — download packages and view private org packages</li>
            <li><code>write</code> — publish new versions (<code>veld push</code>)</li>
            <li><code>delete</code> — unpublish versions (owner only)</li>
          </ul>
          <h3 className={styles.sectionSubtitle}>Use in CI/CD</h3>
          <CodeBlock title=".github/workflows/publish.yml">{`- name: Publish contracts
  env:
    VELD_REGISTRY: https://registry.yourcompany.com
    VELD_TOKEN:    \${{ secrets.VELD_TOKEN }}
  run: |
    veld login --registry $VELD_REGISTRY --token $VELD_TOKEN
    veld push`}
          </CodeBlock>
        </section>

        <section id="registry-config" className={styles.section}>
          <h2 className={styles.sectionTitle}>Config Reference</h2>
          <h3 className={styles.sectionSubtitle}>veld.config.json — registry block</h3>
          <CodeBlock title="veld/veld.config.json" lang="json">{`{
  "registry": {
    "enabled": true,      // false = registry features disabled for this project
    "url":     "http://localhost:8080",   // registry base URL
    "org":     "acme",                   // organisation name (the @scope)
    "package": "auth-service",           // package name
    "version": "1.2.0"                   // version to publish with veld push
  }
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>registry.config.json — server config</h3>
          <CodeBlock title="registry.config.json" lang="json">{`{
  "addr":    ":8080",
  "dsn":     "postgres://user:pass@localhost:5432/dbname?sslmode=disable",
  "storage": "./packages",
  "secret":  "your-jwt-secret-at-least-16-chars"
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>All CLI registry commands</h3>
          <CodeBlock>{`veld login    --registry <url> [--token vtk_…] [--email …] [--password …]
veld logout   [--registry <url>]
veld push     [--org …] [--name …] [--version …] [--registry <url>]
veld pull     @org/name[@version]  [--out <dir>] [--registry <url>]

veld registry info                   # show current registry + logged-in user
veld registry list                   # list all configured registries
veld registry versions @org/name     # list all published versions
veld registry token create --name …  # create a new API token`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Credentials storage</h3>
          <p className={styles.sectionDesc}>
            Tokens are stored in <code>~/.veld/credentials.json</code> with <code>0600</code> permissions.
            Multiple registries are supported simultaneously — each registry URL has its own entry.
          </p>
          <CodeBlock title="~/.veld/credentials.json" lang="json">{`{
  "registries": {
    "http://localhost:8080": {
      "token":    "vtk_…",
      "username": "yourname"
    },
    "https://registry.yourcompany.com": {
      "token":    "vtk_…",
      "username": "yourname"
    }
  }
}`}
          </CodeBlock>
        </section>

      </div>
    </div>
  );
}
