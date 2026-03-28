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
      { id: 'decorators', label: 'Decorators' },
      { id: 'imports', label: 'Import System' },
      { id: 'file-naming', label: 'File Naming' },
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
      { id: 'cli-lint', label: 'veld lint' },
      { id: 'cli-diff', label: 'veld diff' },
      { id: 'cli-export', label: 'veld export' },
      { id: 'cli-setup', label: 'veld setup & ci' },
      { id: 'cli-clean', label: 'veld clean' },
      { id: 'cli-ast', label: 'veld ast' },
    ],
  },
  {
    group: 'Configuration',
    items: [
      { id: 'config-file', label: 'Config File' },
      { id: 'config-fields', label: 'All Config Fields' },
      { id: 'config-aliases', label: 'Import Aliases' },
      { id: 'config-detection', label: 'Auto-Detection' },
      { id: 'config-microservices', label: 'Microservices' },
    ],
  },
  {
    group: 'Generated Output',
    items: [
      { id: 'output-node', label: 'Node.js Backend' },
      { id: 'output-python', label: 'Python Backend' },
      { id: 'output-frontend', label: 'Frontend SDK' },
      { id: 'output-schemas', label: 'Validation Schemas' },
      { id: 'output-routes', label: 'Route Handlers' },
      { id: 'output-server-sdk', label: 'Server SDK' },
    ],
  },
  {
    group: 'Using Generated Code',
    items: [
      { id: 'usage-backend', label: 'Backend Integration' },
      { id: 'usage-frontend', label: 'Frontend SDK Usage' },
      { id: 'usage-websockets', label: 'WebSocket Client' },
      { id: 'usage-path-alias', label: 'Path Aliases' },
    ],
  },
  {
    group: 'Supported Stacks',
    items: [
      { id: 'stacks-backends', label: 'Backend Emitters' },
      { id: 'stacks-frontends', label: 'Frontend Emitters' },
      { id: 'stacks-frameworks', label: 'Framework Strategies' },
    ],
  },
  {
    group: 'AI & Tooling',
    items: [
      { id: 'ai-export', label: 'AI Discoverability' },
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

  useEffect(() => {
    if (location.hash) {
      const el = document.getElementById(location.hash.slice(1));
      if (el) el.scrollIntoView({ behavior: 'smooth' });
    }
  }, [location.hash]);

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
    binary: { install: '# Download from GitHub Releases:\n# https://github.com/Adhamzineldin/Veld/releases\n# Extract and add to your PATH', run: 'veld generate' },
  };

  return (
    <div className={styles.docsLayout}>
      <div
        className={`${styles.mobileOverlay} ${sidebarOpen ? styles.mobileOverlayOpen : ''}`}
        onClick={() => setSidebarOpen(false)}
      />

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

      <button
        className={styles.mobileSidebarToggle}
        onClick={() => setSidebarOpen(!sidebarOpen)}
        aria-label="Toggle docs sidebar"
      >
        {sidebarOpen ? <X size={22} /> : <Menu size={22} />}
      </button>

      <div className={styles.content}>
        <h1 className={styles.pageTitle}>
          Veld <span className={styles.pageTitleGradient}>Documentation</span>
        </h1>
        <p className={styles.pageSubtitle}>
          Everything you need to write <code>.veld</code> contracts and generate typed backends,
          frontend SDKs, validation, WebSocket handlers, and more.
        </p>

        {/* ─── OVERVIEW ─── */}
        <section id="overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>Overview</h2>
          <p className={styles.sectionDesc}>
            <strong>Veld</strong> is a contract-first, multi-stack API code generator. Write
            <code> .veld</code> contract files describing your models, enums, and endpoints. Veld
            generates fully typed backend service interfaces, route handlers with input validation,
            frontend SDKs, OpenAPI specs, database schemas, WebSocket handlers, and more — for any
            stack you choose.
          </p>
          <div className={styles.infoCard}>
            <strong>Zero runtime dependencies</strong> — generated files work out of the box with
            no <code>npm install</code> for type-only usage. Zod (Node.js) and Pydantic (Python)
            validation schemas are generated automatically but require those libraries only when
            you actually use them.
          </div>
          <ul className={styles.featureList}>
            <li>Write your API contract once — generate code for 8+ backend languages and 10+ frontend targets</li>
            <li>Framework agnostic — route handlers accept <code>router: any</code>, works with Express, Fastify, Hono, Flask, or any compatible router</li>
            <li>WebSocket support with typed send/receive messages and auto-reconnect</li>
            <li>Microservices ready — per-module base URLs, workspace polyglot monorepo support</li>
            <li>Deterministic output — same input always produces identical output, safe for CI/CD</li>
            <li>Breaking change detection with interactive prompts and <code>--strict</code> CI mode</li>
            <li>Contract lint, diff, OpenAPI export, GraphQL export, database schema generation</li>
            <li>AI discoverability export (<code>veld export agents</code>) for Claude, Copilot, and Cursor</li>
          </ul>
        </section>

        {/* ─── INSTALLATION ─── */}
        <section id="installation" className={styles.section}>
          <h2 className={styles.sectionTitle}>Installation</h2>
          <p className={styles.sectionDesc}>
            Veld is a standalone Go binary available on all major package managers.
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
            <li><strong>OS:</strong> Windows, macOS, or Linux (amd64 &amp; arm64)</li>
            <li><strong>Runtime:</strong> None — Veld is a self-contained binary compiled from Go</li>
            <li><strong>Node.js:</strong> Required only when using <code>npx</code> or for Node.js backend output</li>
            <li><strong>Python:</strong> Required only for Python backend output</li>
          </ul>
        </section>

        {/* ─── QUICK START ─── */}
        <section id="quickstart" className={styles.section}>
          <h2 className={styles.sectionTitle}>Quick Start</h2>
          <p className={styles.sectionDesc}>Get a typed API running in under five minutes.</p>

          <h3 className={styles.sectionSubtitle}>1. Initialize a project</h3>
          <CodeBlock title="Terminal">
            {`$ mkdir my-api && cd my-api\n$ veld init\n\n✓ Created veld/veld.config.json\n✓ Created veld/app.veld\n✓ Created veld/models/\n✓ Created veld/modules/`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>2. Write your contract</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`model User {
  description: "A registered user"
  id:        uuid
  email:     string
  name:      string
  role:      Role   @default(user)
  tags:      string[]
  createdAt: datetime
}

model CreateUserInput {
  email: string
  name:  string
}

enum Role { admin user guest }

module Users {
  description: "User management"
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

          <h3 className={styles.sectionSubtitle}>3. Generate code</h3>
          <CodeBlock title="Terminal">
            {`$ veld generate\n\n✓ Generated types/users.ts\n✓ Generated interfaces/IUsersService.ts\n✓ Generated routes/users.routes.ts\n✓ Generated schemas/schemas.ts\n✓ Generated client/api.ts\n✓ Generated index.ts\n✓ Generated package.json\n\nDone! 7 files in generated/`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>4. Implement the service interface</h3>
          <CodeBlock title="src/services/UsersService.ts" lang="ts">
            {`import { IUsersService } from '@veld/generated/interfaces/IUsersService';
import { User, CreateUserInput } from '@veld/generated/types';

export class UsersService implements IUsersService {
  async listUsers(): Promise<User[]> {
    return db.users.findMany();
  }

  async getUser(id: string): Promise<User> {
    return db.users.findUniqueOrThrow({ where: { id } });
  }

  async createUser(input: CreateUserInput): Promise<User> {
    return db.users.create({ data: input });
  }

  async deleteUser(id: string): Promise<void> {
    await db.users.delete({ where: { id } });
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>5. Wire routes to your server</h3>
          <CodeBlock title="src/index.ts" lang="ts">
            {`import express from 'express';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = express();
app.use(express.json());
registerUsersRoutes(app, new UsersService());
app.listen(3000);`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>6. Call it from the frontend</h3>
          <CodeBlock title="src/api.ts" lang="ts">
            {`import { VeldApiClient } from '@veld/generated/client/api';

const api = new VeldApiClient('https://api.example.com');

const users = await api.users.listUsers();   // User[]
const user  = await api.users.getUser('abc-123'); // User
await api.users.createUser({ email: 'alice@example.com', name: 'Alice' });`}
          </CodeBlock>
        </section>

        {/* ─── PROJECT STRUCTURE ─── */}
        <section id="project-structure" className={styles.section}>
          <h2 className={styles.sectionTitle}>Project Structure</h2>
          <p className={styles.sectionDesc}>
            After <code>veld init</code>, your project follows this layout. Veld never creates an
            <code> app/</code> directory — the rest of the project is left to you.
          </p>
          <div className={styles.tree}>
{`my-project/
├── veld/                        ← all .veld source (like prisma/)
│   ├── veld.config.json         ← configuration
│   ├── app.veld                 ← entry point (imports other files)
│   ├── models/                  ← model definitions
│   └── modules/                 ← module/action definitions
└── generated/                   ← auto-created on first veld generate
    ├── index.ts
    ├── package.json             ← @veld/generated path alias
    ├── types/                   ← per-module TypeScript interfaces
    ├── interfaces/              ← I{Module}Service.ts contracts
    ├── routes/                  ← route handlers with validation
    ├── schemas/                 ← Zod / Pydantic schemas
    └── client/
        ├── api.ts               ← VeldApiClient with per-module clients
        └── _internal.ts         ← VeldClientConfig, VeldApiError, VeldWebSocket`}
          </div>
          <div className={styles.infoCard}>
            <strong>Safe by design:</strong> Veld never writes outside the <code>--out</code>{' '}
            directory. Your source code is never touched. The <code>generated/</code> directory
            can be deleted and recreated at any time.
          </div>
        </section>

        {/* ─── MODELS ─── */}
        <section id="models" className={styles.section}>
          <h2 className={styles.sectionTitle}>Models</h2>
          <p className={styles.sectionDesc}>
            Models define the data structures in your API. Each model becomes a TypeScript
            interface, Zod schema, and the corresponding type in your backend language.
          </p>
          <CodeBlock title="models/user.model.veld" lang="veld">
            {`model User {
  description: "A registered user in the system"
  id:          uuid
  email:       string
  name:        string
  age?:        int                    // optional field
  tags:        string[]               // array type
  metadata:    Map<string, string>    // key-value map
  role:        Role  @default(user)   // default value
  createdAt:   datetime @serverSet    // set by the server
  displayName: string
  old:         string @deprecated "use displayName"
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Model Syntax</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Syntax</th><th>Meaning</th><th>Example</th></tr>
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
                <td>Array of type</td>
                <td><code>tags: string[]</code></td>
              </tr>
              <tr>
                <td><code>{'field: Map<K, V>'}</code></td>
                <td>Key-value map</td>
                <td><code>{'meta: Map<string, string>'}</code></td>
              </tr>
              <tr>
                <td><code>@default(value)</code></td>
                <td>Default value</td>
                <td><code>role: Role @default(user)</code></td>
              </tr>
              <tr>
                <td><code>@serverSet</code></td>
                <td>Set by server, excluded from inputs</td>
                <td><code>createdAt: datetime @serverSet</code></td>
              </tr>
              <tr>
                <td><code>@deprecated "msg"</code></td>
                <td>Marks field as deprecated</td>
                <td><code>old: string @deprecated "use newField"</code></td>
              </tr>
              <tr>
                <td><code>description: "..."</code></td>
                <td>Model description (OpenAPI/docs)</td>
                <td><code>description: "A user account"</code></td>
              </tr>
            </tbody>
          </table>
        </section>

        {/* ─── ENUMS ─── */}
        <section id="enums" className={styles.section}>
          <h2 className={styles.sectionTitle}>Enums</h2>
          <p className={styles.sectionDesc}>
            Enums define a fixed set of named constants. They generate TypeScript union types,
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
            Modules group related API endpoints. Each module produces a service interface, route
            handler file, and SDK client. Actions map to individual HTTP endpoints or WebSocket
            connections.
          </p>
          <CodeBlock title="modules/users.service.veld" lang="veld">
            {`module Users {
  description: "User management endpoints"
  prefix: /api/v1

  action ListUsers {
    description: "List all users"
    method: GET
    path:   /users
    query:  ListUsersQuery
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

  action Subscribe {
    description: "Real-time user events"
    method: WS
    path:   /users/events/:room
    stream: UserEvent      // server → client
    emit:   ClientCommand  // client → server (optional)
  }

  action GetProfile {
    method: GET
    path:   /users/:id/profile
    output: User
    @deprecated "use GetUser instead"
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Action Fields</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Field</th><th>Required</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr>
                <td><code>method</code></td><td>Yes</td>
                <td><code>GET</code>, <code>POST</code>, <code>PUT</code>, <code>DELETE</code>, <code>PATCH</code>, or <code>WS</code></td>
              </tr>
              <tr>
                <td><code>path</code></td><td>Yes</td>
                <td>URL path, supports path params like <code>/users/:id</code></td>
              </tr>
              <tr>
                <td><code>input</code></td><td>No</td>
                <td>Request body model. Generates Zod/Pydantic validation.</td>
              </tr>
              <tr>
                <td><code>output</code></td><td>No</td>
                <td>Response model. Use <code>User[]</code> for arrays.</td>
              </tr>
              <tr>
                <td><code>query</code></td><td>No</td>
                <td>Query string parameters model</td>
              </tr>
              <tr>
                <td><code>stream</code></td><td>WS only</td>
                <td>Server→client WebSocket message type (required for <code>WS</code>)</td>
              </tr>
              <tr>
                <td><code>emit</code></td><td>No</td>
                <td>Client→server WebSocket message type (optional)</td>
              </tr>
              <tr>
                <td><code>middleware</code></td><td>No</td>
                <td>Middleware name passed as a comment in generated routes</td>
              </tr>
              <tr>
                <td><code>description</code></td><td>No</td>
                <td>Used in OpenAPI docs and generated JSDoc</td>
              </tr>
              <tr>
                <td><code>@deprecated "msg"</code></td><td>No</td>
                <td>Marks action as deprecated; emits JSDoc <code>@deprecated</code></td>
              </tr>
            </tbody>
          </table>

          <h3 className={styles.sectionSubtitle}>HTTP Status Codes</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Method</th><th>With Output</th><th>Without Output</th></tr>
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
            Built-in primitives map to the appropriate type in every target language.
          </p>
          <table className={styles.table}>
            <thead>
              <tr><th>Veld</th><th>TypeScript</th><th>Python</th><th>Zod</th><th>Pydantic</th></tr>
            </thead>
            <tbody>
              <tr><td><code>string</code></td><td><code>string</code></td><td><code>str</code></td><td><code>z.string()</code></td><td><code>str</code></td></tr>
              <tr><td><code>int</code></td><td><code>number</code></td><td><code>int</code></td><td><code>z.number().int()</code></td><td><code>int</code></td></tr>
              <tr><td><code>float</code></td><td><code>number</code></td><td><code>float</code></td><td><code>z.number()</code></td><td><code>float</code></td></tr>
              <tr><td><code>bool</code></td><td><code>boolean</code></td><td><code>bool</code></td><td><code>z.boolean()</code></td><td><code>bool</code></td></tr>
              <tr><td><code>date</code></td><td><code>string</code></td><td><code>str</code></td><td><code>z.string().date()</code></td><td><code>str</code></td></tr>
              <tr><td><code>datetime</code></td><td><code>string</code></td><td><code>str</code></td><td><code>z.string().datetime()</code></td><td><code>str</code></td></tr>
              <tr><td><code>uuid</code></td><td><code>string</code></td><td><code>str</code></td><td><code>z.string().uuid()</code></td><td><code>str</code></td></tr>
              <tr><td><code>T[]</code></td><td><code>T[]</code></td><td><code>List[T]</code></td><td><code>z.array(TSchema)</code></td><td><code>List[T]</code></td></tr>
              <tr>
                <td><code>{'Map<string,V>'}</code></td>
                <td><code>{'Record<string,V>'}</code></td>
                <td><code>{'Dict[str,V]'}</code></td>
                <td><code>z.record(z.string(),V)</code></td>
                <td><code>{'Dict[str,V]'}</code></td>
              </tr>
            </tbody>
          </table>
          <div className={styles.infoCard}>
            <strong>Custom types:</strong> Any <code>PascalCase</code> identifier not in the
            built-in list is treated as a reference to a model or enum defined elsewhere in
            the contract.
          </div>
        </section>

        {/* ─── INHERITANCE ─── */}
        <section id="inheritance" className={styles.section}>
          <h2 className={styles.sectionTitle}>Inheritance (extends)</h2>
          <p className={styles.sectionDesc}>
            Models can extend other models to inherit all fields. Generates TypeScript{' '}
            <code>interface X extends Y</code>, Zod <code>.extend()</code>, and Python class
            inheritance.
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

model AdminUser extends User {
  permissions: string[]
}`}
          </CodeBlock>
          <CodeBlock title="Generated: types/users.ts" lang="ts">
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

export interface AdminUser extends User {
  permissions: string[];
}`}
          </CodeBlock>
          <div className={styles.warningCard}>
            <strong>Circular inheritance</strong> is detected and rejected at validation time.
            A <code>A extends B</code> / <code>B extends A</code> cycle produces a clear error
            with file and line numbers.
          </div>
        </section>

        {/* ─── MAPS ─── */}
        <section id="maps" className={styles.section}>
          <h2 className={styles.sectionTitle}>Maps</h2>
          <p className={styles.sectionDesc}>
            Use <code>{'Map<K, V>'}</code> to define key-value pair fields. Map keys are always
            <code> string</code>; values can be any built-in type or a model/enum reference.
          </p>
          <CodeBlock title="Example" lang="veld">
            {`model Config {
  settings:   Map<string, string>
  featureFlags: Map<string, bool>
  counters:   Map<string, int>
  taggedUsers: Map<string, User>
}`}
          </CodeBlock>
          <CodeBlock title="Generated: TypeScript" lang="ts">
            {`export interface Config {
  settings:    Record<string, string>;
  featureFlags: Record<string, boolean>;
  counters:    Record<string, number>;
  taggedUsers: Record<string, User>;
}`}
          </CodeBlock>
        </section>

        {/* ─── DECORATORS ─── */}
        <section id="decorators" className={styles.section}>
          <h2 className={styles.sectionTitle}>Decorators</h2>
          <p className={styles.sectionDesc}>
            Decorators annotate fields and actions with metadata that affects generated code.
          </p>
          <table className={styles.table}>
            <thead>
              <tr><th>Decorator</th><th>Target</th><th>Effect</th></tr>
            </thead>
            <tbody>
              <tr>
                <td><code>@default(value)</code></td>
                <td>Field</td>
                <td>Sets default value in Zod schema and TypeScript interface</td>
              </tr>
              <tr>
                <td><code>@serverSet</code></td>
                <td>Field</td>
                <td>Marks as server-managed; excluded from input types and client SDK payloads</td>
              </tr>
              <tr>
                <td><code>@deprecated "msg"</code></td>
                <td>Field or Action</td>
                <td>Emits JSDoc <code>@deprecated</code> (TS) or <code>.. deprecated::</code> docstring (Python)</td>
              </tr>
            </tbody>
          </table>
          <CodeBlock title="Usage" lang="veld">
            {`model User {
  role:      Role     @default(user)
  createdAt: datetime @serverSet
  oldField:  string   @deprecated "use displayName instead"
  displayName: string
}

module Auth {
  action OldLogin {
    method: POST
    path: /login-v1
    input: LoginInput
    output: TokenResponse
    @deprecated "use POST /auth/login instead"
  }
}`}
          </CodeBlock>
        </section>

        {/* ─── IMPORTS ─── */}
        <section id="imports" className={styles.section}>
          <h2 className={styles.sectionTitle}>Import System</h2>
          <p className={styles.sectionDesc}>
            Split contracts across multiple files using imports. Veld supports two styles.
          </p>

          <h3 className={styles.sectionSubtitle}>Alias-based imports (recommended)</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`import @models/user
import @models/product
import @modules/users
import @modules/shop`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            Resolved from the project root via the <code>aliases</code> config. The parser
            automatically appends <code>.veld</code> — write <code>import @models/user</code>,
            not <code>import @models/user.veld</code>. Default aliases:{' '}
            <code>@models</code>, <code>@modules</code>, <code>@types</code>, <code>@enums</code>,
            <code> @schemas</code>, <code>@services</code>, <code>@lib</code>, <code>@common</code>,
            <code> @shared</code>.
          </p>

          <h3 className={styles.sectionSubtitle}>Relative imports (legacy)</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`import "./models/user.veld"
import "./modules/users.veld"`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            Resolved relative to the current file. Both styles are supported everywhere: CLI,
            VS Code extension, and JetBrains plugin.
          </p>
        </section>

        {/* ─── FILE NAMING ─── */}
        <section id="file-naming" className={styles.section}>
          <h2 className={styles.sectionTitle}>File Naming Conventions</h2>
          <p className={styles.sectionDesc}>
            Veld supports conventional filename suffixes to make the contents of each file
            immediately clear. The parser appends <code>.veld</code> automatically so imports
            omit it.
          </p>
          <table className={styles.table}>
            <thead>
              <tr><th>Filename</th><th>Contents</th><th>Import as</th></tr>
            </thead>
            <tbody>
              <tr>
                <td><code>user.model.veld</code></td>
                <td>Model/type definitions only</td>
                <td><code>import @models/user.model</code></td>
              </tr>
              <tr>
                <td><code>auth.service.veld</code></td>
                <td>Module/action definitions only</td>
                <td><code>import @modules/auth.service</code></td>
              </tr>
              <tr>
                <td><code>roles.enum.veld</code></td>
                <td>Enum definitions only</td>
                <td><code>import @enums/roles.enum</code></td>
              </tr>
              <tr>
                <td><code>app.veld</code></td>
                <td>Mixed content (entry point)</td>
                <td>n/a (entry point)</td>
              </tr>
            </tbody>
          </table>
          <div className={styles.infoCard}>
            <code>veld lint</code> warns when a file's suffix doesn't match its actual contents —
            for example, a <code>.model.veld</code> file that contains a module definition.
          </div>
        </section>

        {/* ─── WEBSOCKETS ─── */}
        <section id="websockets" className={styles.section}>
          <h2 className={styles.sectionTitle}>WebSockets</h2>
          <p className={styles.sectionDesc}>
            Veld treats WebSocket connections as first-class actions. Use <code>method: WS</code>{' '}
            with a <code>stream</code> type (server→client) and an optional <code>emit</code> type
            (client→server).
          </p>

          <h3 className={styles.sectionSubtitle}>Contract</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`model EventMessage {
  type:    string
  payload: string
  sentAt:  datetime
}

model ClientCommand {
  action: string
  data:   string
}

module Events {
  prefix: /ws

  action Subscribe {
    description: "Subscribe to real-time events in a room"
    method: WS
    path:   /events/:room
    stream: EventMessage    // server → client (required)
    emit:   ClientCommand   // client → server (optional)
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>WS vs REST — key differences</h3>
          <table className={styles.table}>
            <thead>
              <tr><th></th><th>REST action</th><th>WS action</th></tr>
            </thead>
            <tbody>
              <tr><td>Method</td><td><code>GET</code> / <code>POST</code> / …</td><td><code>WS</code></td></tr>
              <tr><td>Request body</td><td><code>input:</code></td><td>not allowed</td></tr>
              <tr><td>Response body</td><td><code>output:</code></td><td>not allowed</td></tr>
              <tr><td>Server push</td><td>not supported</td><td><code>stream:</code> (required)</td></tr>
              <tr><td>Client messages</td><td>not supported</td><td><code>emit:</code> (optional)</td></tr>
              <tr><td>Frontend return</td><td><code>Promise&lt;T&gt;</code></td><td><code>VeldWebSocket&lt;TReceive, TSend&gt;</code></td></tr>
            </tbody>
          </table>

          <h3 className={styles.sectionSubtitle}>Generated backend interface</h3>
          <CodeBlock title="interfaces/IEventsService.ts (generated)" lang="ts">
            {`export interface IEventsService {
  onSubscribeConnect(room: string): void | Promise<void>;
  onSubscribeMessage?(msg: ClientCommand, room: string): void | Promise<void>;
  onSubscribeClose?(room: string): void | Promise<void>;
  onSubscribeError?(err: Error, room: string): void | Promise<void>;
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Generated frontend SDK</h3>
          <CodeBlock title="client/api.ts (generated)" lang="ts">
            {`// connectToSubscribe returns a typed VeldWebSocket
connectToSubscribe(room: string): VeldWebSocket<EventMessage, ClientCommand>`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Frontend usage</h3>
          <CodeBlock title="Usage" lang="ts">
            {`import { VeldApiClient } from '@veld/generated/client/api';

const api = new VeldApiClient('https://api.example.com');

const ws = api.events.connectToSubscribe('general');

ws.onMessage((msg: EventMessage) => {
  console.log(msg.type, msg.payload);
});

ws.send({ action: 'join', data: 'general' } satisfies ClientCommand);

// Close when done
ws.close();`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>VeldWebSocket features</h3>
          <ul className={styles.featureList}>
            <li><strong>Auto-reconnect</strong> with exponential backoff: starts at 1s, caps at 30s</li>
            <li><strong>Typed messages</strong> — <code>onMessage</code> receives the exact <code>stream</code> type</li>
            <li><strong>Typed send</strong> — <code>send()</code> only accepts the <code>emit</code> type</li>
            <li><strong>Chainable</strong> <code>.onMessage()</code> handler</li>
            <li><strong>Path params</strong> auto-become typed parameters on <code>connectToX()</code></li>
          </ul>
          <div className={styles.warningCard}>
            <strong>Validation rules:</strong> <code>stream</code> is required when <code>method: WS</code>.
            {' '}<code>input</code> and <code>output</code> are not allowed on WS actions — use{' '}
            <code>stream</code> and <code>emit</code> instead.
          </div>
        </section>

        {/* ─── CLI OVERVIEW ─── */}
        <section id="cli-overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>CLI Overview</h2>
          <p className={styles.sectionDesc}>
            All commands are available through the single <code>veld</code> binary.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld --help

Core workflow:
  init        Scaffold a new Veld project (language → framework wizard)
  generate    Generate backend and frontend code
  watch       Watch .veld files and auto-regenerate (500ms debounce)
  clean       Remove generated output directory

Quality:
  validate    Parse and validate contract (reports file:line errors)
  lint        Analyse contract for quality issues
  diff        Breaking change detection vs .veld.lock.json

Integration:
  setup       Configure tsconfig paths automatically
  ci          Non-interactive generate + setup (for CI pipelines)

Export:
  export openapi    Export OpenAPI 3.0 spec
  export graphql    Export GraphQL SDL schema
  export schema     Generate database schema (Prisma/SQL)
  export docs       Generate API documentation
  export agents     Export AGENTS.md for AI assistants

Registry:
  login / logout    Authenticate with a registry
  push              Publish contracts to registry
  pull              Download a contract package
  serve             Start self-hosted registry server

Debug:
  ast         Dump parsed AST as JSON
  fmt         Format .veld files
  doctor      Diagnose project health`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld init ─── */}
        <section id="cli-init" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld init</h2>
          <p className={styles.sectionDesc}>
            Scaffolds a new Veld project with an interactive language and framework selection
            wizard. Creates the <code>veld/</code> directory with a config file, entry point,
            and subdirectories.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld init

? Backend language: node
? Backend framework: express
? Frontend target: typescript

✓ Created veld/veld.config.json
✓ Created veld/app.veld
✓ Created veld/models/
✓ Created veld/modules/`}
          </CodeBlock>
          <div className={styles.warningCard}>
            <strong>Safety:</strong> <code>veld init</code> exits with code 1 if the{' '}
            <code>veld/</code> directory already exists. It never overwrites existing files.
          </div>
        </section>

        {/* ─── CLI: veld generate ─── */}
        <section id="cli-generate" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld generate</h2>
          <p className={styles.sectionDesc}>
            The main command. Reads your contract, validates it, checks for breaking changes,
            and generates all output files.
          </p>
          <CodeBlock title="Terminal">
            {`# Use config auto-detection (reads veld.config.json):
$ veld generate

# Specify options explicitly:
$ veld generate \\
  --backend=node \\
  --backend-framework=express \\
  --frontend=typescript \\
  --input=veld/app.veld \\
  --out=./generated

# Preview without writing files:
$ veld generate --dry-run

# Generate all workspace services at once:
$ veld generate --all

# Also emit server-to-server client:
$ veld generate --server-sdk

# CI mode: exit 1 on breaking changes (no prompt):
$ veld generate --strict

# Skip breaking-change check:
$ veld generate --force`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Flags</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Flag</th><th>Default</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td><code>--backend</code></td><td>from config</td><td><code>node</code>, <code>python</code>, <code>go</code>, <code>java</code>, <code>csharp</code>, <code>php</code>, <code>rust</code>, <code>javascript</code></td></tr>
              <tr><td><code>--backend-framework</code></td><td>from config</td><td>e.g. <code>express</code>, <code>flask</code>, <code>chi</code>, <code>spring</code></td></tr>
              <tr><td><code>--frontend</code></td><td>from config</td><td><code>typescript</code>, <code>react</code>, <code>vue</code>, <code>angular</code>, <code>svelte</code>, <code>dart</code>, <code>kotlin</code>, <code>swift</code>, <code>javascript</code>, <code>types-only</code>, <code>none</code></td></tr>
              <tr><td><code>--input</code></td><td>from config</td><td>Entry <code>.veld</code> file</td></tr>
              <tr><td><code>--out</code></td><td>from config</td><td>Output directory</td></tr>
              <tr><td><code>--dry-run</code></td><td><code>false</code></td><td>Preview files without writing</td></tr>
              <tr><td><code>--all</code></td><td><code>false</code></td><td>Generate all workspace services</td></tr>
              <tr><td><code>--server-sdk</code></td><td><code>false</code></td><td>Also emit <code>generated/server-client/api.ts</code></td></tr>
              <tr><td><code>--strict</code></td><td><code>false</code></td><td>Exit 1 on breaking changes (CI mode)</td></tr>
              <tr><td><code>--force</code></td><td><code>false</code></td><td>Skip breaking-change prompt</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── CLI: veld validate ─── */}
        <section id="cli-validate" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld validate</h2>
          <p className={styles.sectionDesc}>
            Parses and validates your contract without generating output. Reports errors with
            file name, line number, and a source snippet.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld validate

✓ Contract is valid (3 models, 1 enum, 2 modules, 5 actions)

# Example error:
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
            Watches <code>.veld</code> files and auto-regenerates with a 500ms debounce.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld watch

Watching for changes in veld/ ...

[12:00:01] Changed: veld/models/user.veld
[12:00:01] Regenerating...
[12:00:01] ✓ Done (7 files in 42ms)`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld lint ─── */}
        <section id="cli-lint" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld lint</h2>
          <p className={styles.sectionDesc}>
            Analyses your contract for quality issues beyond syntax correctness.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld lint

# With non-zero exit code on issues (useful for CI):
$ veld lint --exit-code`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Lint Rules</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Rule</th><th>Severity</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td><code>unused-model</code></td><td>Warning</td><td>Model is defined but never referenced by any action</td></tr>
              <tr><td><code>empty-module</code></td><td>Warning</td><td>Module has no actions</td></tr>
              <tr><td><code>empty-model</code></td><td>Warning</td><td>Model has no fields</td></tr>
              <tr><td><code>duplicate-route</code></td><td>Error</td><td>Two actions share the same method + path</td></tr>
              <tr><td><code>duplicate-action</code></td><td>Error</td><td>Two actions in the same module share a name</td></tr>
              <tr><td><code>missing-description</code></td><td>Warning</td><td>Model or action has no description</td></tr>
              <tr><td><code>deprecated-action</code></td><td>Warning</td><td>Action is marked @deprecated</td></tr>
              <tr><td><code>deprecated-field</code></td><td>Warning</td><td>Field is marked @deprecated</td></tr>
              <tr><td><code>file-naming</code></td><td>Warning</td><td>File suffix doesn't match its contents</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── CLI: veld diff ─── */}
        <section id="cli-diff" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld diff</h2>
          <p className={styles.sectionDesc}>
            Detects breaking changes by comparing the current contract against{' '}
            <code>.veld.lock.json</code> (written by <code>veld generate</code>).
            Supports interactive prompt, <code>--strict</code> (CI), and <code>--force</code> (skip).
          </p>
          <CodeBlock title="Terminal">
            {`$ veld diff

~ Changed model: User
  + Added field: avatarUrl (string?)
  - Removed field: profilePic          ← breaking

~ Changed action: CreateUser
  ~ input type changed: CreateUserInput → CreateUserInputV2  ← breaking

+ Added model: PaymentMethod
+ Added action: Users.ListSessions

? Contract has 2 breaking change(s). Continue? [y/N]`}
          </CodeBlock>
          <CodeBlock title="CI usage">
            {`# Exit 1 if any breaking changes exist:
$ veld generate --strict`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld export ─── */}
        <section id="cli-export" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld export</h2>
          <p className={styles.sectionDesc}>
            Export your contract in various formats for integration with other tools.
          </p>
          <CodeBlock title="Terminal">
            {`# OpenAPI 3.0 JSON
$ veld export openapi
$ veld export openapi -o openapi.json
$ veld export openapi | jq '.paths'

# GraphQL SDL
$ veld export graphql
$ veld export graphql -o schema.graphql

# Database schema
$ veld export schema --format=prisma -o schema.prisma
$ veld export schema --format=sql    -o schema.sql

# API documentation (Markdown)
$ veld export docs -o api-docs.md

# AI assistant export (AGENTS.md)
$ veld export agents
$ veld export agents -o AGENTS.md`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            The legacy short-form commands (<code>veld openapi</code>, <code>veld schema</code>,
            <code> veld docs</code>, <code>veld graphql</code>) are still supported as aliases.
          </p>
        </section>

        {/* ─── CLI: veld setup & ci ─── */}
        <section id="cli-setup" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld setup &amp; veld ci</h2>

          <h3 className={styles.sectionSubtitle}>veld setup</h3>
          <p className={styles.sectionDesc}>
            Automatically configures <code>tsconfig.json</code> with the{' '}
            <code>@veld/generated</code> path alias so you can import generated types without
            manual config edits.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld setup

✓ Added "@veld/*": ["./generated/*"] to tsconfig.json`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>veld ci</h3>
          <p className={styles.sectionDesc}>
            Non-interactive <code>generate + setup</code> in one step. Designed for CI/CD
            pipelines where you want to regenerate and configure paths without any prompts.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld ci

✓ Generated 7 files
✓ Configured tsconfig.json`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld clean ─── */}
        <section id="cli-clean" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld clean</h2>
          <p className={styles.sectionDesc}>
            Removes the generated output directory and the <code>.veld.lock.json</code> file.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld clean\n\n✓ Removed generated/\n✓ Removed .veld.lock.json`}
          </CodeBlock>
        </section>

        {/* ─── CLI: veld ast ─── */}
        <section id="cli-ast" className={styles.section}>
          <h2 className={styles.sectionTitle}>veld ast</h2>
          <p className={styles.sectionDesc}>
            Dumps the parsed AST as JSON. Useful for debugging or building custom tooling on
            top of Veld's parser.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld ast | jq '.models[0]'
{
  "name": "User",
  "fields": [
    { "name": "id",    "type": "uuid",   "optional": false },
    { "name": "email", "type": "string", "optional": false }
  ]
}`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG FILE ─── */}
        <section id="config-file" className={styles.section}>
          <h2 className={styles.sectionTitle}>Configuration File</h2>
          <p className={styles.sectionDesc}>
            Created automatically by <code>veld init</code>. Contains defaults for all CLI
            commands — any flag passed to the CLI overrides the corresponding config field.
          </p>
          <CodeBlock title="veld/veld.config.json" lang="json">
            {`{
  "input":             "app.veld",
  "backend":           "node",
  "backendFramework":  "express",
  "frontend":          "typescript",
  "frontendFramework": "react",
  "out":               "../generated",
  "baseUrl":           "https://api.example.com",
  "description":       "My project API",
  "validate":          true,
  "serverSdk":         false,
  "services": {
    "Auth":   "https://auth.api.example.com",
    "Users":  "https://users.api.example.com"
  },
  "workspace": [],
  "aliases": {
    "models":  "models",
    "modules": "modules"
  },
  "postGenerate": "npm run format",
  "registry": {
    "enabled": false,
    "url":     "",
    "org":     "",
    "package": "",
    "version": "0.1.0"
  }
}`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG FIELDS ─── */}
        <section id="config-fields" className={styles.section}>
          <h2 className={styles.sectionTitle}>All Config Fields</h2>
          <table className={styles.table}>
            <thead>
              <tr><th>Field</th><th>Type</th><th>Default</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr>
                <td><code>input</code></td><td>string</td><td><em>required</em></td>
                <td>Entry <code>.veld</code> file (relative to config location)</td>
              </tr>
              <tr>
                <td><code>backend</code></td><td>string</td><td><code>"node"</code></td>
                <td><code>node</code>, <code>python</code>, <code>go</code>, <code>java</code>, <code>csharp</code>, <code>php</code>, <code>rust</code>, <code>javascript</code></td>
              </tr>
              <tr>
                <td><code>backendFramework</code></td><td>string</td><td><code>""</code></td>
                <td>e.g. <code>express</code>, <code>flask</code>, <code>fastapi</code>, <code>chi</code>, <code>gin</code>, <code>spring</code>, <code>laravel</code></td>
              </tr>
              <tr>
                <td><code>frontend</code></td><td>string</td><td><code>"typescript"</code></td>
                <td><code>typescript</code>, <code>react</code>, <code>vue</code>, <code>angular</code>, <code>svelte</code>, <code>dart</code>, <code>kotlin</code>, <code>swift</code>, <code>javascript</code>, <code>types-only</code>, <code>none</code></td>
              </tr>
              <tr>
                <td><code>frontendFramework</code></td><td>string</td><td><code>""</code></td>
                <td>Additional hint for frontend emitter (e.g. <code>react</code>)</td>
              </tr>
              <tr>
                <td><code>out</code></td><td>string</td><td><code>"./generated"</code></td>
                <td>Output directory (relative to config location)</td>
              </tr>
              <tr>
                <td><code>baseUrl</code></td><td>string</td><td><code>""</code></td>
                <td>Baked into frontend SDK. Empty = reads <code>VELD_API_URL</code> at runtime</td>
              </tr>
              <tr>
                <td><code>description</code></td><td>string</td><td><code>""</code></td>
                <td>Human-readable description used in OpenAPI and AGENTS.md export</td>
              </tr>
              <tr>
                <td><code>validate</code></td><td>bool</td><td><code>true</code></td>
                <td>Run validation before generation</td>
              </tr>
              <tr>
                <td><code>serverSdk</code></td><td>bool</td><td><code>false</code></td>
                <td>Also emit <code>generated/server-client/api.ts</code> (server-to-server client)</td>
              </tr>
              <tr>
                <td><code>services</code></td><td>object</td><td><code>{'{}'}</code></td>
                <td>Per-module base URL overrides for microservices (module name → URL)</td>
              </tr>
              <tr>
                <td><code>workspace</code></td><td>array</td><td><code>[]</code></td>
                <td>Polyglot monorepo entries — each is an independent service config</td>
              </tr>
              <tr>
                <td><code>aliases</code></td><td>object</td><td>built-in</td>
                <td>Custom <code>@alias</code> → relative directory mappings</td>
              </tr>
              <tr>
                <td><code>postGenerate</code></td><td>string</td><td><code>""</code></td>
                <td>Shell command to run after successful generation (e.g. <code>npm run format</code>)</td>
              </tr>
              <tr>
                <td><code>registry</code></td><td>object</td><td>—</td>
                <td>Registry publishing config (see Cloud Registry section)</td>
              </tr>
            </tbody>
          </table>
        </section>

        {/* ─── CONFIG ALIASES ─── */}
        <section id="config-aliases" className={styles.section}>
          <h2 className={styles.sectionTitle}>Import Aliases</h2>
          <p className={styles.sectionDesc}>
            Aliases map short <code>@name</code> prefixes to directories relative to the
            config file. Built-in defaults cover the most common layouts.
          </p>
          <CodeBlock title="veld.config.json" lang="json">
            {`{
  "aliases": {
    "models":   "models",
    "modules":  "modules",
    "types":    "types",
    "enums":    "enums",
    "schemas":  "schemas",
    "services": "services",
    "lib":      "lib",
    "common":   "common",
    "shared":   "shared"
  }
}`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            Add custom aliases for non-standard layouts. They are merged with the built-in defaults:
          </p>
          <CodeBlock title="Custom aliases" lang="json">
            {`{
  "aliases": {
    "auth":     "services/auth",
    "payments": "features/payments/contracts"
  }
}`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG DETECTION ─── */}
        <section id="config-detection" className={styles.section}>
          <h2 className={styles.sectionTitle}>Config Auto-Detection</h2>
          <p className={styles.sectionDesc}>
            When <code>veld generate</code> is run with no flags, it looks for the config
            file in this order:
          </p>
          <ol className={styles.featureList}>
            <li><code>./veld.config.json</code></li>
            <li><code>./veld/veld.config.json</code></li>
          </ol>
          <p className={styles.sectionDesc}>
            CLI flags always win over config values:
          </p>
          <CodeBlock title="Terminal">
            {`# Config says backend=node — override with python for this run:
$ veld generate --backend=python`}
          </CodeBlock>
        </section>

        {/* ─── CONFIG: MICROSERVICES ─── */}
        <section id="config-microservices" className={styles.section}>
          <h2 className={styles.sectionTitle}>Microservices & Workspaces</h2>
          <p className={styles.sectionDesc}>
            Veld has first-class support for microservices: per-module base URLs in the contract,
            a <code>services</code> map in config, and a <code>workspace</code> array for polyglot
            monorepos.
          </p>

          <h3 className={styles.sectionSubtitle}>Per-module base URL (in contract)</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`module Auth {
  description: "Authentication service"
  baseUrl: https://auth.svc.example.com
  prefix: /api/auth

  action Login {
    method: POST
    path:   /login
    input:  LoginInput
    output: TokenResponse
  }
}

module Orders {
  description: "Orders service"
  baseUrl: https://orders.svc.example.com
  prefix: /api/orders

  action ListOrders {
    method: GET
    path:   /
    output: Order[]
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>services map (in config)</h3>
          <p className={styles.sectionDesc}>
            Alternatively, set per-module base URLs in config — useful when URLs change between
            environments:
          </p>
          <CodeBlock title="veld.config.json" lang="json">
            {`{
  "description": "E-commerce platform",
  "backend":  "node",
  "frontend": "typescript",
  "baseUrl":  "https://api.example.com",
  "services": {
    "Auth":   "https://auth.svc.example.com",
    "Orders": "https://orders.svc.example.com"
  }
}`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            <code>baseUrl</code> is the global fallback. The <code>services</code> map overrides it
            per-module. The generated SDK routes each module's calls to its own base URL automatically.
          </p>

          <h3 className={styles.sectionSubtitle}>Workspace — polyglot monorepo</h3>
          <p className={styles.sectionDesc}>
            Use <code>workspace</code> to define multiple independent services in one repo, each
            with its own backend language and output directory:
          </p>
          <CodeBlock title="veld.config.json" lang="json">
            {`{
  "workspace": [
    {
      "name":    "auth",
      "input":   "veld/auth.veld",
      "backend": "node",
      "out":     "services/auth/generated"
    },
    {
      "name":    "orders",
      "input":   "veld/orders.veld",
      "backend": "go",
      "out":     "services/orders/generated"
    },
    {
      "name":    "notifications",
      "input":   "veld/notifications.veld",
      "backend": "python",
      "out":     "services/notifications/generated"
    }
  ]
}`}
          </CodeBlock>
          <CodeBlock title="Terminal">
            {`# Generate all services in one command:
$ veld generate --all

✓ [auth]          7 files → services/auth/generated/
✓ [orders]        5 files → services/orders/generated/
✓ [notifications] 6 files → services/notifications/generated/`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Server SDK</h3>
          <p className={styles.sectionDesc}>
            When <code>serverSdk: true</code> (or <code>--server-sdk</code>), Veld also generates
            a typed server-to-server client. Unlike the frontend SDK, it requires an explicit
            <code> baseUrl</code> and never reads from environment variables.
          </p>
          <CodeBlock title="Terminal">
            {`$ veld generate --server-sdk`}
          </CodeBlock>
          <div className={styles.tree}>
{`generated/
└── server-client/
    └── api.ts    ← requires explicit baseUrl, no VELD_API_URL fallback`}
          </div>
        </section>

        {/* ─── OUTPUT: NODE ─── */}
        <section id="output-node" className={styles.section}>
          <h2 className={styles.sectionTitle}>Node.js Backend Output</h2>
          <div className={styles.tree}>
{`generated/
├── index.ts                       # Barrel export
├── package.json                   # @veld/generated alias
├── types/
│   ├── users.ts                   # Types owned by Users module
│   ├── auth.ts                    # Types owned by Auth module + re-exports shared
│   └── index.ts                   # Barrel re-export of all type files
├── interfaces/
│   ├── IUsersService.ts           # REST service contract
│   └── IEventsService.ts          # WS lifecycle: onConnect, onMessage?, onClose?, onError?
├── routes/
│   ├── users.routes.ts            # HTTP: try/catch, Zod validation, status codes
│   └── events.routes.ts           # WS: mountEventsWS(server, service)
├── schemas/
│   └── schemas.ts                 # Zod schemas (extends → .extend())
└── client/
    ├── api.ts                     # VeldApiClient with per-module clients + connectToX()
    └── _internal.ts               # VeldClientConfig, VeldApiError, VeldWebSocket`}
          </div>
          <p className={styles.sectionDesc}>
            Each type is defined in exactly one file (the first module to reference it). Other
            modules re-export shared types. All generated files begin with{' '}
            <code>// AUTO-GENERATED BY VELD — DO NOT EDIT</code>.
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
          <p className={styles.sectionDesc}>
            All generated Python files begin with{' '}
            <code># AUTO-GENERATED BY VELD — DO NOT EDIT</code>.
          </p>
        </section>

        {/* ─── OUTPUT: FRONTEND ─── */}
        <section id="output-frontend" className={styles.section}>
          <h2 className={styles.sectionTitle}>Frontend SDK</h2>
          <p className={styles.sectionDesc}>
            The generated frontend SDK uses native <code>fetch</code> — no axios, no runtime
            dependencies. The new <code>VeldApiClient</code> class provides per-module clients
            and WebSocket connect methods.
          </p>

          <h3 className={styles.sectionSubtitle}>VeldApiClient initialization</h3>
          <CodeBlock title="Usage" lang="ts">
            {`import { VeldApiClient } from '@veld/generated/client/api';

// String shorthand — baseUrl only:
const api = new VeldApiClient('https://api.example.com');

// Full config — headers, auth, etc.:
const api = new VeldApiClient({
  baseUrl: 'https://api.example.com',
  headers: { Authorization: 'Bearer ' + token },
});

// No args — reads VELD_API_URL from environment:
const api = new VeldApiClient();`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Calling REST endpoints</h3>
          <CodeBlock title="Usage" lang="ts">
            {`// Per-module sub-clients (when services map is configured):
const users  = await api.users.listUsers();   // User[]
const user   = await api.users.getUser('abc-123');  // User
await api.users.createUser({ email: 'a@b.com', name: 'Alice' });
await api.users.deleteUser('abc-123');

// Error handling:
import { VeldApiError } from '@veld/generated/client/_internal';

try {
  await api.users.getUser('nonexistent');
} catch (err) {
  if (err instanceof VeldApiError) {
    console.error(err.status); // e.g. 404
    console.error(err.body);   // parsed response body
  }
}`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>SDK features</h3>
          <ul className={styles.featureList}>
            <li><strong>VeldApiError</strong> with typed <code>status: number</code> and <code>body: unknown</code></li>
            <li><strong>Path param interpolation:</strong> <code>/users/:id</code> → typed <code>id: string</code> parameter</li>
            <li><strong>All HTTP methods:</strong> <code>get()</code>, <code>post()</code>, <code>put()</code>, <code>patch()</code>, <code>del()</code></li>
            <li><strong>Per-module sub-clients</strong> when <code>services</code> map is configured</li>
            <li><strong>WS connect methods</strong> for every <code>method: WS</code> action</li>
            <li><strong>Zero dependencies</strong> — only native <code>fetch</code> and <code>WebSocket</code></li>
          </ul>
        </section>

        {/* ─── OUTPUT: SCHEMAS ─── */}
        <section id="output-schemas" className={styles.section}>
          <h2 className={styles.sectionTitle}>Validation Schemas</h2>
          <p className={styles.sectionDesc}>
            Schemas are generated automatically and used inside route handlers. You can also
            import and use them directly.
          </p>
          <h3 className={styles.sectionSubtitle}>Node.js (Zod)</h3>
          <CodeBlock title="schemas/schemas.ts" lang="ts">
            {`import { z } from 'zod';

export const RoleSchema = z.enum(['admin', 'user', 'guest']);

export const UserSchema = z.object({
  id:        z.string().uuid(),
  email:     z.string(),
  name:      z.string(),
  role:      RoleSchema.default('user'),
  createdAt: z.string().datetime(),
});

// Extends generate .extend():
export const AdminUserSchema = UserSchema.extend({
  permissions: z.array(z.string()),
});

export const CreateUserInputSchema = z.object({
  email: z.string(),
  name:  z.string(),
});`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Python (Pydantic)</h3>
          <CodeBlock title="schemas/schemas.py">
            {`from pydantic import BaseModel
from typing import Optional, List

class UserSchema(BaseModel):
    id:         str
    email:      str
    name:       str
    role:       str = 'user'
    created_at: str

class AdminUserSchema(UserSchema):
    permissions: List[str]

class CreateUserInputSchema(BaseModel):
    email: str
    name:  str`}
          </CodeBlock>
        </section>

        {/* ─── OUTPUT: ROUTES ─── */}
        <section id="output-routes" className={styles.section}>
          <h2 className={styles.sectionTitle}>Route Handlers</h2>
          <p className={styles.sectionDesc}>
            Generated route handlers include try/catch wrapping, Zod input validation, correct
            HTTP status codes, and path parameter extraction. The <code>router</code> parameter
            accepts <code>any</code> — wire in Express, Fastify, Hono, or any router with{' '}
            <code>.get()</code>/<code>.post()</code>.
          </p>
          <CodeBlock title="routes/users.routes.ts (generated)" lang="ts">
            {`// AUTO-GENERATED BY VELD — DO NOT EDIT
import { IUsersService } from '../interfaces/IUsersService';
import { CreateUserInputSchema } from '../schemas/schemas';

export function registerUsersRoutes(router: any, service: IUsersService) {

  router.get('/api/v1/users', async (req: any, res: any) => {
    try {
      const result = await service.listUsers();
      res.status(200).json(result);
    } catch (err) {
      res.status(500).json({ error: 'Internal server error' });
    }
  });

  router.get('/api/v1/users/:id', async (req: any, res: any) => {
    try {
      const result = await service.getUser(req.params.id);
      res.status(200).json(result);
    } catch (err) {
      res.status(500).json({ error: 'Internal server error' });
    }
  });

  router.post('/api/v1/users', async (req: any, res: any) => {
    try {
      const input = CreateUserInputSchema.parse(req.body);
      const result = await service.createUser(input);
      res.status(201).json(result);
    } catch (err: any) {
      if (err?.name === 'ZodError') {
        return res.status(400).json({ errors: err.issues });
      }
      res.status(500).json({ error: 'Internal server error' });
    }
  });

  router.delete('/api/v1/users/:id', async (req: any, res: any) => {
    try {
      await service.deleteUser(req.params.id);
      res.status(204).end();
    } catch (err) {
      res.status(500).json({ error: 'Internal server error' });
    }
  });
}`}
          </CodeBlock>
        </section>

        {/* ─── OUTPUT: SERVER SDK ─── */}
        <section id="output-server-sdk" className={styles.section}>
          <h2 className={styles.sectionTitle}>Server SDK</h2>
          <p className={styles.sectionDesc}>
            Enabled with <code>serverSdk: true</code> in config or <code>--server-sdk</code> on
            the CLI. Generates a typed server-to-server HTTP client. Unlike the frontend SDK,
            it requires an explicit <code>baseUrl</code> and never reads from{' '}
            <code>VELD_API_URL</code>.
          </p>
          <div className={styles.tree}>
{`generated/
└── server-client/
    └── api.ts    ← server-to-server typed client`}
          </div>
          <CodeBlock title="Usage" lang="ts">
            {`import { VeldServerClient } from '@veld/generated/server-client/api';

// baseUrl is required — no env fallback
const internal = new VeldServerClient('https://users.svc.internal');

const user = await internal.users.getUser('abc-123');`}
          </CodeBlock>
          <div className={styles.infoCard}>
            Use the server SDK when one microservice needs to call another. The frontend SDK
            is intended for browser/mobile clients; the server SDK is for server-to-server calls.
          </div>
        </section>

        {/* ─── USAGE: BACKEND ─── */}
        <section id="usage-backend" className={styles.section}>
          <h2 className={styles.sectionTitle}>Backend Integration</h2>
          <p className={styles.sectionDesc}>
            Implement the generated service interface, then pass it to the generated route
            registration function. Veld never touches your business logic files.
          </p>

          <h3 className={styles.sectionSubtitle}>1. Implement the service</h3>
          <CodeBlock title="src/services/UsersService.ts" lang="ts">
            {`import { IUsersService } from '@veld/generated/interfaces/IUsersService';
import { User, CreateUserInput } from '@veld/generated/types';

export class UsersService implements IUsersService {
  async listUsers(): Promise<User[]> {
    return await db.users.findMany();
  }

  async getUser(id: string): Promise<User> {
    const user = await db.users.findUnique({ where: { id } });
    if (!user) throw new Error('Not found');
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

          <h3 className={styles.sectionSubtitle}>2. Wire to your router</h3>
          <CodeBlock title="Express" lang="ts">
            {`import express from 'express';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = express();
app.use(express.json());
registerUsersRoutes(app, new UsersService());
app.listen(3000);`}
          </CodeBlock>
          <CodeBlock title="Fastify" lang="ts">
            {`import Fastify from 'fastify';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = Fastify();
registerUsersRoutes(app, new UsersService());
app.listen({ port: 3000 });`}
          </CodeBlock>
          <CodeBlock title="Hono" lang="ts">
            {`import { Hono } from 'hono';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { UsersService } from './services/UsersService';

const app = new Hono();
registerUsersRoutes(app, new UsersService());
export default app;`}
          </CodeBlock>
        </section>

        {/* ─── USAGE: FRONTEND ─── */}
        <section id="usage-frontend" className={styles.section}>
          <h2 className={styles.sectionTitle}>Frontend SDK Usage</h2>
          <CodeBlock title="src/api.ts" lang="ts">
            {`import { VeldApiClient } from '@veld/generated/client/api';
import { VeldApiError } from '@veld/generated/client/_internal';

const api = new VeldApiClient({
  baseUrl: import.meta.env.VITE_API_URL,
  headers: { 'X-App-Version': '2.0' },
});

// All methods are fully typed
const users = await api.users.listUsers();          // User[]
const user  = await api.users.getUser('abc-123');   // User
const newUser = await api.users.createUser({
  email: 'alice@example.com',
  name:  'Alice',
});

// Type-safe error handling
try {
  await api.users.getUser('bad-id');
} catch (err) {
  if (err instanceof VeldApiError) {
    if (err.status === 404) {
      console.log('User not found');
    }
  }
}`}
          </CodeBlock>
        </section>

        {/* ─── USAGE: WEBSOCKET CLIENT ─── */}
        <section id="usage-websockets" className={styles.section}>
          <h2 className={styles.sectionTitle}>WebSocket Client</h2>
          <p className={styles.sectionDesc}>
            WebSocket actions generate typed <code>connectToX()</code> methods on the
            module client. The return type is <code>{'VeldWebSocket<TReceive, TSend>'}</code>.
          </p>
          <CodeBlock title="Usage" lang="ts">
            {`import { VeldApiClient } from '@veld/generated/client/api';
import type { EventMessage, ClientCommand } from '@veld/generated/types';

const api = new VeldApiClient('https://api.example.com');

// Path params become typed arguments:
// connectToSubscribe(room: string): VeldWebSocket<EventMessage, ClientCommand>
const ws = api.events.connectToSubscribe('general');

// Typed message handler:
ws.onMessage((msg: EventMessage) => {
  console.log(msg.type, msg.payload);
});

// Typed send — only accepts ClientCommand:
ws.send({ action: 'ping', data: '' } satisfies ClientCommand);

// Close the connection:
ws.close();`}
          </CodeBlock>

          <h3 className={styles.sectionSubtitle}>Auto-reconnect</h3>
          <CodeBlock title="VeldWebSocket reconnect behaviour" lang="ts">
            {`// VeldWebSocket reconnects automatically on disconnect.
// Backoff schedule: 1s → 2s → 4s → 8s → 16s → 30s (capped)
//
// To disable reconnect:
const ws = api.events.connectToSubscribe('general', { reconnect: false });

// To handle reconnect events:
ws.onReconnect(() => {
  console.log('Reconnected');
  ws.send({ action: 'rejoin', data: 'general' });
});`}
          </CodeBlock>
        </section>

        {/* ─── USAGE: PATH ALIAS ─── */}
        <section id="usage-path-alias" className={styles.section}>
          <h2 className={styles.sectionTitle}>Path Aliases</h2>
          <p className={styles.sectionDesc}>
            The generated <code>package.json</code> sets up the <code>@veld/generated</code>{' '}
            alias. Run <code>veld setup</code> to automatically patch your{' '}
            <code>tsconfig.json</code>, or add it manually:
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
          <CodeBlock title="Usage" lang="ts">
            {`import type { User }           from '@veld/generated/types';
import { IUsersService }      from '@veld/generated/interfaces/IUsersService';
import { registerUsersRoutes } from '@veld/generated/routes/users.routes';
import { VeldApiClient }       from '@veld/generated/client/api';`}
          </CodeBlock>
        </section>

        {/* ─── STACKS: BACKENDS ─── */}
        <section id="stacks-backends" className={styles.section}>
          <h2 className={styles.sectionTitle}>Backend Emitters</h2>
          <p className={styles.sectionDesc}>
            Eight backend targets. Each generates types, service interfaces, route handlers,
            and validation schemas in the target language.
          </p>
          <table className={styles.table}>
            <thead>
              <tr><th>Flag</th><th>Language</th><th>Validation</th><th>Default style</th></tr>
            </thead>
            <tbody>
              <tr><td><code>node</code></td><td>TypeScript (Node.js)</td><td>Zod</td><td>Router-agnostic TS (<code>router: any</code>)</td></tr>
              <tr><td><code>javascript</code></td><td>JavaScript (Node.js)</td><td>Zod</td><td>Same, no TypeScript</td></tr>
              <tr><td><code>python</code></td><td>Python</td><td>Pydantic</td><td>Typed ABC interfaces</td></tr>
              <tr><td><code>go</code></td><td>Go</td><td>Built-in</td><td>net/http 1.22 handlers</td></tr>
              <tr><td><code>rust</code></td><td>Rust</td><td>Serde</td><td>Typed service traits</td></tr>
              <tr><td><code>java</code></td><td>Java</td><td>Jakarta</td><td>Service interfaces + build.gradle</td></tr>
              <tr><td><code>csharp</code></td><td>C#</td><td>DataAnnotations</td><td>ASP.NET Core service interfaces</td></tr>
              <tr><td><code>php</code></td><td>PHP</td><td>Built-in</td><td>Service contracts</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── STACKS: FRONTENDS ─── */}
        <section id="stacks-frontends" className={styles.section}>
          <h2 className={styles.sectionTitle}>Frontend Emitters</h2>
          <table className={styles.table}>
            <thead>
              <tr><th>Flag</th><th>Output</th><th>Aliases</th></tr>
            </thead>
            <tbody>
              <tr><td><code>typescript</code></td><td>Fetch-based SDK with <code>VeldApiClient</code></td><td><code>react</code></td></tr>
              <tr><td><code>vue</code></td><td>Vue Composables wrapping TS SDK</td><td>—</td></tr>
              <tr><td><code>angular</code></td><td>Angular services wrapping TS SDK</td><td>—</td></tr>
              <tr><td><code>svelte</code></td><td>Svelte stores/functions wrapping TS SDK</td><td>—</td></tr>
              <tr><td><code>dart</code></td><td>Dart http client SDK</td><td><code>flutter</code></td></tr>
              <tr><td><code>kotlin</code></td><td>Kotlin client SDK</td><td>—</td></tr>
              <tr><td><code>swift</code></td><td>Swift URLSession SDK</td><td>—</td></tr>
              <tr><td><code>javascript</code></td><td>Plain JS fetch SDK (no TypeScript)</td><td>—</td></tr>
              <tr><td><code>types-only</code></td><td>Types with no SDK logic</td><td>—</td></tr>
              <tr><td><code>none</code></td><td>No frontend SDK generated</td><td>—</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── STACKS: FRAMEWORKS ─── */}
        <section id="stacks-frameworks" className={styles.section}>
          <h2 className={styles.sectionTitle}>Framework Strategies</h2>
          <p className={styles.sectionDesc}>
            For backends that support multiple frameworks, set <code>backendFramework</code> in
            config or pass <code>--backend-framework</code> to the CLI.
          </p>
          <table className={styles.table}>
            <thead>
              <tr><th>Backend</th><th>Framework values</th><th>Default (no framework)</th></tr>
            </thead>
            <tbody>
              <tr><td><code>node</code></td><td><code>express</code></td><td>Router-agnostic TS (<code>router: any</code>)</td></tr>
              <tr><td><code>javascript</code></td><td><code>express</code></td><td>Router-agnostic JS</td></tr>
              <tr><td><code>python</code></td><td><code>flask</code>, <code>fastapi</code></td><td>Pure typed ABC interfaces</td></tr>
              <tr><td><code>go</code></td><td><code>chi</code>, <code>gin</code></td><td>net/http 1.22 handlers</td></tr>
              <tr><td><code>rust</code></td><td><code>axum</code></td><td>Typed service traits</td></tr>
              <tr><td><code>java</code></td><td><code>spring</code></td><td>Service interfaces + build.gradle</td></tr>
              <tr><td><code>csharp</code></td><td><code>aspnet</code></td><td>Service interfaces</td></tr>
              <tr><td><code>php</code></td><td><code>laravel</code></td><td>Service contracts</td></tr>
            </tbody>
          </table>
          <CodeBlock title="Config" lang="json">
            {`{
  "backend":          "node",
  "backendFramework": "express"
}`}
          </CodeBlock>
          <CodeBlock title="CLI">
            {`$ veld generate --backend=node --backend-framework=express
$ veld generate --backend=python --backend-framework=fastapi
$ veld generate --backend=go --backend-framework=chi`}
          </CodeBlock>
        </section>

        {/* ─── AI DISCOVERABILITY ─── */}
        <section id="ai-export" className={styles.section}>
          <h2 className={styles.sectionTitle}>AI Discoverability</h2>
          <p className={styles.sectionDesc}>
            <code>veld export agents</code> generates a compact Markdown file with your full API
            contract, types, and SDK usage examples — optimised for AI assistants (Claude, Copilot,
            Cursor) to ingest in a single read.
          </p>
          <CodeBlock title="Terminal">
            {`# Print to stdout:
$ veld export agents

# Write to file:
$ veld export agents -o AGENTS.md`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            The generated <code>AGENTS.md</code> includes:
          </p>
          <ul className={styles.featureList}>
            <li>All models and their fields with types</li>
            <li>All enums</li>
            <li>All modules with actions (method, path, input, output)</li>
            <li>Ready-to-use SDK code examples for each action</li>
            <li>WebSocket connect/send/receive examples for WS actions</li>
            <li>The <code>description</code> from <code>veld.config.json</code> as a header</li>
          </ul>
          <div className={styles.infoCard}>
            Commit <code>AGENTS.md</code> to your repo root so AI assistants automatically
            discover your API surface without reading generated TypeScript or contract files.
          </div>
        </section>

        {/* ─── EDITOR: VS CODE ─── */}
        <section id="editor-vscode" className={styles.section}>
          <h2 className={styles.sectionTitle}>VS Code Extension</h2>
          <p className={styles.sectionDesc}>
            First-class editing for <code>.veld</code> files with real-time diagnostics,
            completions, and navigation.
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
            <li>Real-time error and warning diagnostics</li>
            <li>Autocomplete for keywords, types, and model/enum references</li>
            <li>Hover information showing field types and descriptions</li>
            <li>Go-to-definition for model and enum references</li>
            <li>Code snippets: <code>model</code>, <code>module</code>, <code>action</code>, <code>enum</code></li>
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
            <li>Click Install and restart</li>
          </ol>
          <h3 className={styles.sectionSubtitle}>Features</h3>
          <ul className={styles.featureList}>
            <li>Syntax highlighting and code folding</li>
            <li>Error highlighting with quick-fix actions</li>
            <li>Autocomplete for Veld keywords, types, and references</li>
            <li>Navigate to definition</li>
          </ul>
        </section>

        {/* ─── EDITOR: LSP ─── */}
        <section id="editor-lsp" className={styles.section}>
          <h2 className={styles.sectionTitle}>LSP Server</h2>
          <p className={styles.sectionDesc}>
            Veld ships a built-in Language Server Protocol server — any editor can connect to
            it for diagnostics, completions, hover, and go-to-definition.
          </p>
          <CodeBlock title="Start the LSP server">
            {`$ veld lsp

# Communicates via JSON-RPC 2.0 over stdin/stdout.
# Configure your editor to run "veld lsp" as the language server
# for .veld files.`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Supported capabilities</h3>
          <ul className={styles.featureList}>
            <li><code>textDocument/publishDiagnostics</code> — real-time error reporting</li>
            <li><code>textDocument/completion</code> — keyword, type, and reference completions</li>
            <li><code>textDocument/hover</code> — type info and descriptions on hover</li>
            <li><code>textDocument/definition</code> — go-to-definition for model/enum references</li>
          </ul>
          <h3 className={styles.sectionSubtitle}>Neovim setup</h3>
          <CodeBlock title="init.lua">
            {`vim.api.nvim_create_autocmd('FileType', {
  pattern = 'veld',
  callback = function()
    vim.lsp.start({
      name = 'veld',
      cmd  = { 'veld', 'lsp' },
      root_dir = vim.fs.dirname(
        vim.fs.find({ 'veld.config.json' }, { upward = true })[1]
      ),
    })
  end,
})`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: OVERVIEW ─── */}
        <section id="registry-overview" className={styles.section}>
          <h2 className={styles.sectionTitle}>Cloud Registry</h2>
          <p className={styles.sectionDesc}>
            The Veld Registry is a typed contract package registry — like npm, but for{' '}
            <code>.veld</code> files. Teams publish contracts once; every consuming repo pulls
            exact versions. Generated SDKs stay in sync across services with no manual
            copy-paste.
          </p>
          <p className={styles.sectionDesc}>
            The registry is <strong>fully self-hostable</strong> — a single Go binary and a
            PostgreSQL database. The CLI workflow is identical whether you use a self-hosted
            instance or a hosted registry.
          </p>
          <h3 className={styles.sectionSubtitle}>Team workflow</h3>
          <CodeBlock title="Terminal">
            {`# Team A publishes their auth contracts:
cd auth-service/veld
veld push                           # → @acme/auth@1.2.0

# Team B consumes them in a separate repo:
veld pull @acme/auth@1.2.0          # → veld/packages/@acme/auth/
veld generate                        # → fully typed SDK from pulled contracts`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: SELF-HOST ─── */}
        <section id="registry-selfhost" className={styles.section}>
          <h2 className={styles.sectionTitle}>Self-Hosting</h2>
          <p className={styles.sectionDesc}>
            The registry server is built into the main <code>veld</code> binary. Start it with
            <code> veld serve</code>.
          </p>
          <h3 className={styles.sectionSubtitle}>1. Create a config file</h3>
          <CodeBlock title="registry.config.json" lang="json">
            {`{
  "addr":    ":8080",
  "dsn":     "postgres://veld:secret@localhost:5432/veld?sslmode=disable",
  "storage": "./packages",
  "secret":  "run: openssl rand -hex 32",
  "base_url": "https://registry.example.com"
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>2. Create the database and start</h3>
          <CodeBlock title="Terminal">
            {`createdb veld
veld serve --config registry.config.json

# Registry running on http://localhost:8080
# Web UI available at http://localhost:8080/`}
          </CodeBlock>
          <p className={styles.sectionDesc}>
            The schema is applied automatically on first start. Config priority (highest wins):
          </p>
          <CodeBlock>
            {`CLI flag  >  env var  >  registry.config.json  >  default

# Keep secrets out of the file:
VELD_SECRET=mysecret veld serve   # reads rest from registry.config.json`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>All config fields</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Field</th><th>Env var</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td><code>addr</code></td><td><code>VELD_ADDR</code></td><td>Listen address (default <code>:8080</code>)</td></tr>
              <tr><td><code>dsn</code></td><td><code>VELD_DSN</code></td><td>PostgreSQL connection string</td></tr>
              <tr><td><code>storage</code></td><td><code>VELD_STORAGE</code></td><td>Local directory for tarball storage</td></tr>
              <tr><td><code>secret</code></td><td><code>VELD_SECRET</code></td><td>JWT signing secret (min 16 chars)</td></tr>
              <tr><td><code>base_url</code></td><td><code>VELD_BASE_URL</code></td><td>Public URL shown in web UI</td></tr>
              <tr><td><code>smtp.host</code></td><td><code>SMTP_HOST</code></td><td>SMTP host for email verification</td></tr>
              <tr><td><code>smtp.port</code></td><td>—</td><td>SMTP port (default 587)</td></tr>
              <tr><td><code>smtp.username</code></td><td><code>SMTP_USERNAME</code></td><td>SMTP username</td></tr>
              <tr><td><code>smtp.password</code></td><td><code>SMTP_PASSWORD</code></td><td>SMTP password</td></tr>
              <tr><td><code>smtp.from</code></td><td><code>SMTP_FROM</code></td><td>From address for outgoing email</td></tr>
            </tbody>
          </table>
        </section>

        {/* ─── REGISTRY: LOGIN ─── */}
        <section id="registry-login" className={styles.section}>
          <h2 className={styles.sectionTitle}>Login &amp; Auth</h2>
          <p className={styles.sectionDesc}>
            Credentials are stored in <code>~/.veld/credentials.json</code> with{' '}
            <code>0600</code> permissions. Multiple registries can be configured simultaneously.
          </p>
          <h3 className={styles.sectionSubtitle}>Interactive login</h3>
          <CodeBlock title="Terminal">
            {`veld login --registry http://localhost:8080
# Email: you@example.com
# Password: ••••••••
# ✓ Logged in to http://localhost:8080 as yourname`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Token login (CI/CD)</h3>
          <CodeBlock title="Terminal">
            {`veld login --registry http://localhost:8080 --token vtk_xxxxxxxxxxxxxxxx
# ✓ Logged in to http://localhost:8080 as yourname`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Check identity &amp; logout</h3>
          <CodeBlock title="Terminal">
            {`veld registry info          # show current registry + logged-in user
veld registry list          # show all configured registries
veld logout
veld logout --registry http://other-registry.com`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: PUSH ─── */}
        <section id="registry-push" className={styles.section}>
          <h2 className={styles.sectionTitle}>Publishing Contracts</h2>
          <p className={styles.sectionDesc}>
            <code>veld push</code> packs <code>.veld</code> files and <code>veld.config.json</code>{' '}
            into a gzip-compressed, SHA-256 signed tarball and uploads it to the registry.
            You must be an <strong>admin</strong> or <strong>owner</strong> of the organisation.
          </p>
          <h3 className={styles.sectionSubtitle}>Configure in veld.config.json</h3>
          <CodeBlock title="veld/veld.config.json" lang="json">
            {`{
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
          <CodeBlock title="Terminal">
            {`# Reads org/package/version/url from veld.config.json:
veld push

# Override on the CLI:
veld push --org acme --name auth-service --version 1.2.0
veld push --registry http://localhost:8080 --org acme --name auth --version 2.0.0

# Output:
# ⬡  Packing contracts from ./veld...
# ⬡  Publishing @acme/auth-service@1.2.0 (4.2 kB)...
# ✓  Published @acme/auth-service@1.2.0`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: PULL ─── */}
        <section id="registry-pull" className={styles.section}>
          <h2 className={styles.sectionTitle}>Installing Contracts</h2>
          <p className={styles.sectionDesc}>
            <code>veld pull</code> downloads a versioned package, verifies its SHA-256 checksum,
            and extracts it to <code>veld/packages/@org/name/</code>. Pulled contracts are imported
            exactly like local files.
          </p>
          <CodeBlock title="Terminal">
            {`veld pull @acme/auth-service             # latest version
veld pull @acme/auth-service@1.2.0      # exact version
veld pull @acme/auth-service@1.2.0 --out veld/packages   # custom dir`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Declare dependencies in config</h3>
          <CodeBlock title="veld/veld.config.json" lang="json">
            {`{
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
          <h3 className={styles.sectionSubtitle}>Import pulled contracts</h3>
          <CodeBlock title="veld/app.veld" lang="veld">
            {`import @acme/auth-service/UserModel
import @acme/shared-types/PaginationMeta

module Orders {
  prefix: /api/orders

  action ListOrders {
    method: GET
    path:   /
    output: PaginationMeta   // from pulled package
  }
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>List versions</h3>
          <CodeBlock title="Terminal">
            {`veld registry versions @acme/auth-service
# @acme/auth-service — 3 version(s):
#   v1.2.0
#   v1.1.0
#   v1.0.0  [deprecated: use 1.2.0]`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: TEAMS ─── */}
        <section id="registry-teams" className={styles.section}>
          <h2 className={styles.sectionTitle}>Teams &amp; Organisations</h2>
          <p className={styles.sectionDesc}>
            Packages are scoped under organisations (the <code>@scope</code>). Each org has
            members with roles that control who can publish, manage members, and delete packages.
          </p>
          <h3 className={styles.sectionSubtitle}>Create an organisation</h3>
          <CodeBlock title="Terminal">
            {`# Via web UI: http://localhost:8080/#/orgs → New Organisation

# REST API:
curl -X POST http://localhost:8080/api/v1/orgs \\
  -H "Authorization: Bearer vtk_…" \\
  -H "Content-Type: application/json" \\
  -d '{"name":"acme","display_name":"ACME Corp"}'`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Role permissions</h3>
          <table className={styles.table}>
            <thead>
              <tr><th>Action</th><th>Member</th><th>Admin</th><th>Owner</th></tr>
            </thead>
            <tbody>
              <tr><td>Pull private packages</td><td>✓</td><td>✓</td><td>✓</td></tr>
              <tr><td>Publish new versions</td><td>✗</td><td>✓</td><td>✓</td></tr>
              <tr><td>Deprecate versions</td><td>✗</td><td>✓</td><td>✓</td></tr>
              <tr><td>Manage members</td><td>✗</td><td>✓</td><td>✓</td></tr>
              <tr><td>Unpublish versions</td><td>✗</td><td>✗</td><td>✓</td></tr>
            </tbody>
          </table>
          <h3 className={styles.sectionSubtitle}>Add a member</h3>
          <CodeBlock title="Terminal">
            {`curl -X POST http://localhost:8080/api/v1/orgs/acme/members \\
  -H "Authorization: Bearer vtk_…" \\
  -d '{"username":"alice","role":"admin"}'`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: TOKENS ─── */}
        <section id="registry-tokens" className={styles.section}>
          <h2 className={styles.sectionTitle}>API Tokens</h2>
          <p className={styles.sectionDesc}>
            Tokens are prefixed with <code>vtk_</code> and stored as SHA-256 hashes — the plain
            text is shown <strong>only once</strong> at creation. Use tokens for CI/CD pipelines.
          </p>
          <h3 className={styles.sectionSubtitle}>Create a token</h3>
          <CodeBlock title="Terminal">
            {`# Via web UI: http://localhost:8080/#/tokens → New Token

# Via CLI:
veld registry token create --name ci-deploy --scopes read,write`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Token scopes</h3>
          <ul className={styles.featureList}>
            <li><code>read</code> — download packages, view private org packages</li>
            <li><code>write</code> — publish new versions (<code>veld push</code>)</li>
            <li><code>delete</code> — unpublish versions (owner only)</li>
          </ul>
          <h3 className={styles.sectionSubtitle}>CI/CD usage</h3>
          <CodeBlock title=".github/workflows/publish.yml">
            {`- name: Publish contracts
  env:
    VELD_REGISTRY: https://registry.yourcompany.com
    VELD_TOKEN:    \${{ secrets.VELD_TOKEN }}
  run: |
    veld login --registry $VELD_REGISTRY --token $VELD_TOKEN
    veld push`}
          </CodeBlock>
        </section>

        {/* ─── REGISTRY: CONFIG REFERENCE ─── */}
        <section id="registry-config" className={styles.section}>
          <h2 className={styles.sectionTitle}>Registry Config Reference</h2>
          <h3 className={styles.sectionSubtitle}>veld.config.json — registry block</h3>
          <CodeBlock title="veld/veld.config.json" lang="json">
            {`{
  "registry": {
    "enabled": true,
    "url":     "http://localhost:8080",
    "org":     "acme",
    "package": "auth-service",
    "version": "1.2.0"
  }
}`}
          </CodeBlock>
          <h3 className={styles.sectionSubtitle}>Credentials file</h3>
          <CodeBlock title="~/.veld/credentials.json" lang="json">
            {`{
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
          <h3 className={styles.sectionSubtitle}>All registry CLI commands</h3>
          <CodeBlock>
            {`veld login    --registry <url> [--token vtk_…]
veld logout   [--registry <url>]
veld push     [--org …] [--name …] [--version …] [--registry <url>]
veld pull     @org/name[@version]  [--out <dir>] [--registry <url>]
veld serve    [--config registry.config.json] [--addr :8080] [--dsn "postgres://…"]

veld registry info
veld registry list
veld registry versions @org/name
veld registry token create --name … --scopes read,write`}
          </CodeBlock>
        </section>

      </div>
    </div>
  );
}
