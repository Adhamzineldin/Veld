import { useState, useCallback, useRef } from 'react';
import { motion } from 'framer-motion';
import { Play, RotateCcw, Server, Monitor, FileCode, Code, Wrench } from 'lucide-react';
import { highlightTS, highlightVeld } from '../SyntaxHighlighter';
import styles from './LiveDemo.module.css';

// ── Veld mini-compiler (client-side simulation) ──────────────────────────

interface ParsedModel {
  name: string;
  fields: { name: string; type: string; optional: boolean; decorator?: string }[];
}

interface ParsedEnum {
  name: string;
  values: string[];
}

interface ParsedAction {
  name: string;
  method: string;
  path: string;
  input?: string;
  output?: string;
  errors?: string[];
}

interface ParsedModule {
  name: string;
  prefix: string;
  actions: ParsedAction[];
}

interface ParseResult {
  models: ParsedModel[];
  enums: ParsedEnum[];
  modules: ParsedModule[];
  errors: string[];
}

function parseVeld(source: string): ParseResult {
  const models: ParsedModel[] = [];
  const enums: ParsedEnum[] = [];
  const modules: ParsedModule[] = [];
  const errors: string[] = [];

  const lines = source.split('\n');
  let i = 0;

  while (i < lines.length) {
    const line = lines[i].trim();

    if (line === '' || line.startsWith('//') || line.startsWith('import')) {
      i++;
      continue;
    }

    // ── Model ──
    const modelMatch = line.match(/^model\s+(\w+)\s*\{/);
    if (modelMatch) {
      const model: ParsedModel = { name: modelMatch[1], fields: [] };
      i++;
      while (i < lines.length) {
        const fl = lines[i].trim();
        if (fl === '}') { i++; break; }
        if (fl === '' || fl.startsWith('//') || fl.startsWith('description')) { i++; continue; }
        const fieldMatch = fl.match(/^(\w+)(\?)?\s*:\s*(\S+)(?:\s+(@\w+(?:\([^)]*\))?))?/);
        if (fieldMatch) {
          model.fields.push({
            name: fieldMatch[1],
            type: fieldMatch[3],
            optional: fieldMatch[2] === '?',
            decorator: fieldMatch[4],
          });
        }
        i++;
      }
      models.push(model);
      continue;
    }

    // ── Enum ──
    const enumMatch = line.match(/^enum\s+(\w+)\s*\{([^}]*)\}/);
    if (enumMatch) {
      enums.push({
        name: enumMatch[1],
        values: enumMatch[2].trim().split(/\s+/),
      });
      i++;
      continue;
    }
    // multi-line enum
    const enumStartMatch = line.match(/^enum\s+(\w+)\s*\{/);
    if (enumStartMatch) {
      const vals: string[] = [];
      i++;
      while (i < lines.length) {
        const el = lines[i].trim();
        if (el === '}') { i++; break; }
        if (el) vals.push(...el.split(/\s+/).filter(Boolean));
        i++;
      }
      enums.push({ name: enumStartMatch[1], values: vals });
      continue;
    }

    // ── Module ──
    const moduleMatch = line.match(/^module\s+(\w+)\s*\{/);
    if (moduleMatch) {
      const mod: ParsedModule = { name: moduleMatch[1], prefix: '', actions: [] };
      i++;
      let depth = 1;
      let currentAction: Partial<ParsedAction> | null = null;

      while (i < lines.length && depth > 0) {
        const ml = lines[i].trim();

        if (ml === '}') {
          depth--;
          if (depth === 1 && currentAction && currentAction.name) {
            mod.actions.push(currentAction as ParsedAction);
            currentAction = null;
          }
          i++;
          continue;
        }

        const prefixMatch = ml.match(/^prefix:\s*(\S+)/);
        if (prefixMatch && depth === 1) {
          mod.prefix = prefixMatch[1];
          i++;
          continue;
        }

        const actionMatch = ml.match(/^action\s+(\w+)\s*\{/);
        if (actionMatch) {
          currentAction = { name: actionMatch[1], method: 'GET', path: '/', errors: [] };
          depth++;
          i++;
          continue;
        }

        if (currentAction) {
          const methodMatch = ml.match(/^method:\s*(\w+)/);
          if (methodMatch) currentAction.method = methodMatch[1];

          const pathMatch = ml.match(/^path:\s*(\S+)/);
          if (pathMatch) currentAction.path = pathMatch[1];

          const inputMatch = ml.match(/^input:\s*(\w+)/);
          if (inputMatch) currentAction.input = inputMatch[1];

          const outputMatch = ml.match(/^output:\s*(\w[\w[\]]*)/);
          if (outputMatch) currentAction.output = outputMatch[1];

          const errorsMatch = ml.match(/^errors:\s*\[([^\]]+)\]/);
          if (errorsMatch) currentAction.errors = errorsMatch[1].split(',').map((e) => e.trim());
        }

        i++;
      }
      modules.push(mod);
      continue;
    }

    // Unknown line
    if (line && !line.startsWith('//')) {
      errors.push(`Line ${i + 1}: Unexpected "${line.substring(0, 30)}..."`);
    }
    i++;
  }

  return { models, enums, modules, errors };
}

// ── Semantic validation ──

const BUILTIN_TYPES = new Set([
  'string', 'int', 'float', 'bool', 'date', 'datetime', 'uuid',
]);

const VALID_METHODS = new Set(['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'WS']);

function validateParsed(parsed: ParseResult): string[] {
  const errs: string[] = [];

  // Collect all known type names
  const knownTypes = new Set<string>(BUILTIN_TYPES);
  for (const m of parsed.models) knownTypes.add(m.name);
  for (const e of parsed.enums) knownTypes.add(e.name);

  // Validate model field types
  for (const model of parsed.models) {
    for (const field of model.fields) {
      const baseType = field.type.replace(/\[\]$/, '');
      if (!knownTypes.has(baseType)) {
        errs.push(`Model "${model.name}": field "${field.name}" has unknown type "${baseType}"`);
      }
    }
  }

  // Validate module actions
  for (const mod of parsed.modules) {
    if (!mod.prefix) {
      errs.push(`Module "${mod.name}": missing prefix`);
    }

    if (mod.actions.length === 0) {
      errs.push(`Module "${mod.name}": no actions defined`);
    }

    for (const action of mod.actions) {
      // Validate method
      if (!VALID_METHODS.has(action.method)) {
        errs.push(`Action "${action.name}": invalid method "${action.method}" (use GET, POST, PUT, DELETE, PATCH)`);
      }

      // Validate input type
      if (action.input) {
        const baseInput = action.input.replace(/\[\]$/, '');
        if (!knownTypes.has(baseInput)) {
          errs.push(`Action "${action.name}": input type "${action.input}" is not defined — declare a model for it`);
        }
      }

      // Validate output type
      if (action.output) {
        const baseOutput = action.output.replace(/\[\]$/, '');
        if (!knownTypes.has(baseOutput)) {
          errs.push(`Action "${action.name}": output type "${action.output}" is not defined — declare a model for it`);
        }
      }

      // Validate path
      if (!action.path || !action.path.startsWith('/')) {
        errs.push(`Action "${action.name}": path must start with "/"`);
      }

      // GET with input body is a warning
      if (action.method === 'GET' && action.input) {
        errs.push(`Action "${action.name}": GET actions should not have an input body — use query params instead`);
      }
    }
  }

  return errs;
}

// ── Type mapper ──

function veldTypeToTS(t: string): string {
  const map: Record<string, string> = {
    string: 'string', int: 'number', float: 'number', bool: 'boolean',
    date: 'string', datetime: 'string', uuid: 'string',
  };
  if (t.endsWith('[]')) return `${veldTypeToTS(t.slice(0, -2))}[]`;
  return map[t] || t;
}

// ── Generate frontend SDK ──

function generateFrontendSDK(parsed: ParseResult): string {
  let out = '// AUTO-GENERATED BY VELD — DO NOT EDIT\n\n';

  // Types
  for (const model of parsed.models) {
    out += `export interface ${model.name} {\n`;
    for (const f of model.fields) {
      const opt = f.optional ? '?' : '';
      out += `  ${f.name}${opt}: ${veldTypeToTS(f.type)};\n`;
    }
    out += '}\n\n';
  }

  for (const en of parsed.enums) {
    out += `export type ${en.name} = ${en.values.map((v) => `'${v}'`).join(' | ')};\n\n`;
  }

  // SDK
  for (const mod of parsed.modules) {
    out += `// ── ${mod.name} SDK ──\n\n`;
    out += `export const ${mod.name} = {\n`;
    for (const action of mod.actions) {
      const camelName = action.name.charAt(0).toLowerCase() + action.name.slice(1);
      const pathParams = (action.path.match(/:(\w+)/g) || []).map((p) => p.slice(1));
      const params: string[] = pathParams.map((p) => `${p}: string`);
      if (action.input) params.push(`data: ${action.input}`);

      const fullPath = mod.prefix + action.path;
      const interpolatedPath = fullPath.replace(/:(\w+)/g, '${$1}');
      const returnType = action.output ? `Promise<${veldTypeToTS(action.output)}>` : 'Promise<void>';

      out += `  async ${camelName}(${params.join(', ')}): ${returnType} {\n`;
      out += `    const res = await fetch(\`\${BASE_URL}${interpolatedPath}\``;

      if (action.method !== 'GET') {
        out += `, {\n`;
        out += `        method: '${action.method}',\n`;
        if (action.input) {
          out += `      headers: { 'Content-Type': 'application/json' },\n`;
          out += `      body: JSON.stringify(data),\n`;
        }
        out += `    }`;
      }
      out += `);\n`;
      out += `    if (!res.ok) throw new VeldApiError(res.status, await res.json());\n`;
      if (action.output) {
        out += `    return res.json();\n`;
      }
      out += `  },\n\n`;
    }
    out += '};\n';
  }

  return out;
}

// ── Generate backend routes ──

function generateBackendRoutes(parsed: ParseResult): string {
  let out = '// AUTO-GENERATED BY VELD — DO NOT EDIT\n\n';

  // Interfaces
  for (const mod of parsed.modules) {
    const iface = `I${mod.name}Service`;
    out += `export interface ${iface} {\n`;
    for (const action of mod.actions) {
      const camelName = action.name.charAt(0).toLowerCase() + action.name.slice(1);
      const pathParams = (action.path.match(/:(\w+)/g) || []).map((p) => p.slice(1));
      const params: string[] = pathParams.map((p) => `${p}: string`);
      if (action.input) params.push(`input: ${action.input}`);
      const ret = action.output ? veldTypeToTS(action.output) : 'void';
      out += `  ${camelName}(${params.join(', ')}): Promise<${ret}>;\n`;
    }
    out += '}\n\n';
  }

  // Routes
  for (const mod of parsed.modules) {
    const iface = `I${mod.name}Service`;
    out += `export function register${mod.name}Routes(\n`;
    out += `  router: any,\n`;
    out += `  service: ${iface}\n`;
    out += `) {\n`;

    for (const action of mod.actions) {
      const camelName = action.name.charAt(0).toLowerCase() + action.name.slice(1);
      const method = action.method.toLowerCase();
      const fullPath = mod.prefix + action.path;
      const pathParams = (action.path.match(/:(\w+)/g) || []).map((p) => p.slice(1));
      const statusCode = action.method === 'POST' ? 201 : action.method === 'DELETE' && !action.output ? 204 : 200;

      out += `\n  router.${method}('${fullPath}', async (req, res) => {\n`;
      out += `    try {\n`;

      const serviceArgs: string[] = pathParams.map((p) => `req.params.${p}`);
      if (action.input) {
        out += `      const input = ${action.input}Schema.parse(req.body);\n`;
        serviceArgs.push('input');
      }

      if (action.output) {
        out += `      const result = await service.${camelName}(${serviceArgs.join(', ')});\n`;
        out += `      res.status(${statusCode}).json(result);\n`;
      } else {
        out += `      await service.${camelName}(${serviceArgs.join(', ')});\n`;
        out += `      res.status(${statusCode}).end();\n`;
      }

      out += `    } catch (err) {\n`;
      if (action.input) {
        out += `      if (err.name === 'ZodError') {\n`;
        out += `        return res.status(400).json({ errors: err.issues });\n`;
        out += `      }\n`;
      }
      out += `      res.status(500).json({ error: 'Internal server error' });\n`;
      out += `    }\n`;
      out += `  });\n`;
    }

    out += '}\n';
  }

  return out;
}

// ── Generate SDK usage example ──

function generateSDKUsage(parsed: ParseResult): string {
  let out = '// ── How to use the generated frontend SDK ──\n\n';
  out += `import { api } from '@veld/client';\n`;
  out += `import { VeldApiError } from '@veld/client';\n\n`;

  for (const mod of parsed.modules) {
    out += `// ── ${mod.name} ──\n\n`;

    for (const action of mod.actions) {
      const camelName = action.name.charAt(0).toLowerCase() + action.name.slice(1);
      const pathParams = (action.path.match(/:(\w+)/g) || []).map((p) => p.slice(1));
      const hasErrors = action.errors && action.errors.length > 0;

      // Build example call
      const args: string[] = [];
      for (const p of pathParams) {
        args.push(`'example-${p}'`);
      }
      if (action.input) {
        // Build example object from input model
        const inputModel = parsed.models.find((m) => m.name === action.input);
        if (inputModel) {
          const fields = inputModel.fields.map((f) => {
            const val = exampleValue(f.type, f.name, parsed);
            return `    ${f.name}: ${val},`;
          });
          args.push(`{\n${fields.join('\n')}\n  }`);
        } else {
          args.push('{ /* ... */ }');
        }
      }

      if (hasErrors) {
        out += `// ${action.name} — with error handling\n`;
        out += `try {\n`;
        if (action.output) {
          out += `  const result = await api.${mod.name}.${camelName}(${args.join(', ')});\n`;
          out += `  console.log(result);\n`;
        } else {
          out += `  await api.${mod.name}.${camelName}(${args.join(', ')});\n`;
          out += `  console.log('${action.name} succeeded');\n`;
        }
        out += `} catch (err) {\n`;
        out += `  if (err instanceof VeldApiError) {\n`;
        for (const errCode of action.errors!) {
          out += `    if (err.status === 404) {\n`;
          out += `      console.error('${errCode}:', err.body);\n`;
          out += `    }\n`;
        }
        out += `  }\n`;
        out += `}\n\n`;
      } else {
        out += `// ${action.name}\n`;
        if (action.output) {
          out += `const ${camelName}Result = await api.${mod.name}.${camelName}(${args.join(', ')});\n`;
          out += `console.log(${camelName}Result);\n`;
          if (action.output.endsWith('[]')) {
            out += `// ^? ${veldTypeToTS(action.output)}\n`;
            out += `${camelName}Result.forEach(item => console.log(item));\n`;
          } else {
            out += `// ^? ${veldTypeToTS(action.output)}\n`;
          }
        } else {
          out += `await api.${mod.name}.${camelName}(${args.join(', ')});\n`;
          out += `// ^? void (${action.method === 'DELETE' ? '204 No Content' : '200 OK'})\n`;
        }
        out += '\n';
      }
    }
  }

  return out;
}

// ── Generate backend implementation example ──

function generateBackendImpl(parsed: ParseResult): string {
  let out = '// ── How to implement the generated service interface ──\n\n';

  for (const mod of parsed.modules) {
    const iface = `I${mod.name}Service`;
    out += `import { ${iface} } from '@veld/generated/interfaces/${iface}';\n`;
  }
  // Import types
  const allTypes = new Set<string>();
  for (const mod of parsed.modules) {
    for (const action of mod.actions) {
      if (action.input) allTypes.add(action.input);
      if (action.output) allTypes.add(action.output.replace(/\[\]$/, ''));
    }
  }
  if (allTypes.size > 0) {
    out += `import { ${[...allTypes].join(', ')} } from '@veld/generated/types';\n`;
  }
  out += '\n';

  for (const mod of parsed.modules) {
    const iface = `I${mod.name}Service`;
    const className = `${mod.name}Service`;

    out += `export class ${className} implements ${iface} {\n\n`;

    for (const action of mod.actions) {
      const camelName = action.name.charAt(0).toLowerCase() + action.name.slice(1);
      const pathParams = (action.path.match(/:(\w+)/g) || []).map((p) => p.slice(1));
      const params: string[] = pathParams.map((p) => `${p}: string`);
      if (action.input) params.push(`input: ${action.input}`);
      const ret = action.output ? veldTypeToTS(action.output) : 'void';

      out += `  async ${camelName}(${params.join(', ')}): Promise<${ret}> {\n`;
      out += `    // TODO: implement your business logic here\n`;

      // Generate example implementation body
      if (action.method === 'GET' && action.output) {
        if (action.output.endsWith('[]')) {
          out += `    // Example: fetch all from database\n`;
          out += `    const items = await db.${mod.name.toLowerCase()}.findMany();\n`;
          out += `    return items;\n`;
        } else if (pathParams.length > 0) {
          out += `    // Example: fetch by ${pathParams[0]}\n`;
          out += `    const record = await db.${mod.name.toLowerCase()}.findUnique({\n`;
          out += `      where: { ${pathParams[0]} },\n`;
          out += `    });\n`;
          if (action.errors && action.errors.length > 0) {
            out += `    if (!record) {\n`;
            out += `      throw new Error('${action.errors[0]}');\n`;
            out += `    }\n`;
          }
          out += `    return record;\n`;
        }
      } else if (action.method === 'POST' && action.input) {
        out += `    // Example: create new record\n`;
        out += `    const created = await db.${mod.name.toLowerCase()}.create({\n`;
        out += `      data: input,\n`;
        out += `    });\n`;
        if (action.output) {
          out += `    return created;\n`;
        }
      } else if (action.method === 'PUT' || action.method === 'PATCH') {
        out += `    // Example: update record\n`;
        if (pathParams.length > 0) {
          out += `    const updated = await db.${mod.name.toLowerCase()}.update({\n`;
          out += `      where: { ${pathParams[0]} },\n`;
          out += `      data: input,\n`;
          out += `    });\n`;
          if (action.output) {
            out += `    return updated;\n`;
          }
        }
      } else if (action.method === 'DELETE') {
        if (pathParams.length > 0) {
          out += `    // Example: delete by ${pathParams[0]}\n`;
          out += `    await db.${mod.name.toLowerCase()}.delete({\n`;
          out += `      where: { ${pathParams[0]} },\n`;
          out += `    });\n`;
        }
      }

      out += `  }\n\n`;
    }

    out += '}\n\n';

    // Wiring example
    out += `// ── Wire it up ──\n\n`;
    out += `import express from 'express';\n`;
    out += `import { register${mod.name}Routes } from '@veld/generated/routes/${mod.name.toLowerCase()}.routes';\n\n`;
    out += `const app = express();\n`;
    out += `app.use(express.json());\n\n`;
    out += `const ${mod.name.toLowerCase()}Service = new ${className}();\n`;
    out += `register${mod.name}Routes(app, ${mod.name.toLowerCase()}Service);\n\n`;
    out += `app.listen(3000, () => {\n`;
    out += `  console.log('Server running on http://localhost:3000');\n`;
    out += `});\n`;
  }

  return out;
}

// ── Example value helper ──

function exampleValue(type: string, fieldName: string, parsed: ParseResult): string {
  if (type.endsWith('[]')) return `[${exampleValue(type.slice(0, -2), fieldName, parsed)}]`;

  // Check enums
  const enumDef = parsed.enums.find((e) => e.name === type);
  if (enumDef && enumDef.values.length > 0) return `'${enumDef.values[0]}'`;

  switch (type) {
    case 'string': return `'example-${fieldName}'`;
    case 'uuid': return `'550e8400-e29b-41d4-a716-446655440000'`;
    case 'int': return '1';
    case 'float': return '9.99';
    case 'bool': return 'true';
    case 'date': return `'2026-03-05'`;
    case 'datetime': return `'2026-03-05T12:00:00Z'`;
    default: return '{ /* ... */ }';
  }
}

// ── Presets ──

const presets: Record<string, { label: string; code: string }> = {
  users: {
    label: 'User CRUD',
    code: `model User {
  id:    uuid
  email: string
  name:  string
  role:  Role   @default(user)
}

model CreateUserInput {
  email: string
  name:  string
  role:  Role   @default(user)
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
    errors: [NotFound]
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
}`,
  },
  blog: {
    label: 'Blog API',
    code: `model Post {
  id:        uuid
  title:     string
  body:      string
  authorId:  uuid
  published: bool    @default(false)
  createdAt: datetime
}

model CreatePostInput {
  title:    string
  body:     string
  authorId: uuid
}

model Comment {
  id:       uuid
  postId:   uuid
  author:   string
  content:  string
}

model AddCommentInput {
  author:  string
  content: string
}

module Blog {
  prefix: /api/v1

  action GetPost {
    method: GET
    path:   /posts/:id
    output: Post
    errors: [NotFound]
  }

  action CreatePost {
    method: POST
    path:   /posts
    input:  CreatePostInput
    output: Post
  }

  action ListComments {
    method: GET
    path:   /posts/:postId/comments
    output: Comment[]
  }

  action AddComment {
    method: POST
    path:   /posts/:postId/comments
    input:  AddCommentInput
    output: Comment
  }
}`,
  },
  ecommerce: {
    label: 'E-Commerce',
    code: `model Product {
  id:       uuid
  name:     string
  price:    float
  currency: string
  stock:    int
  category: Category
}

enum Category { electronics clothing food books }

model Order {
  id:         uuid
  productId:  uuid
  quantity:   int
  total:      float
  status:     OrderStatus
}

model PlaceOrderInput {
  productId: uuid
  quantity:  int
}

enum OrderStatus { pending confirmed shipped delivered }

module Shop {
  prefix: /api/v1

  action ListProducts {
    method: GET
    path:   /products
    output: Product[]
  }

  action GetProduct {
    method: GET
    path:   /products/:id
    output: Product
    errors: [NotFound]
  }

  action PlaceOrder {
    method: POST
    path:   /orders
    input:  PlaceOrderInput
    output: Order
  }

  action GetOrder {
    method: GET
    path:   /orders/:id
    output: Order
    errors: [NotFound]
  }
}`,
  },
};

// ── Component ──

type OutputTab = 'frontend' | 'backend' | 'sdk-usage' | 'backend-impl';

export default function LiveDemo() {
  const [veldSource, setVeldSource] = useState(presets.users.code);
  const [activeTab, setActiveTab] = useState<OutputTab>('frontend');
  const [generated, setGenerated] = useState<{
    frontend: string;
    backend: string;
    sdkUsage: string;
    backendImpl: string;
  } | null>(null);
  const [parseErrors, setParseErrors] = useState<string[]>([]);
  const [generating, setGenerating] = useState(false);
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLPreElement>(null);

  const syncScroll = useCallback(() => {
    if (editorRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = editorRef.current.scrollTop;
      highlightRef.current.scrollLeft = editorRef.current.scrollLeft;
    }
  }, []);

  const handleGenerate = useCallback(() => {
    setGenerating(true);
    setTimeout(() => {
      const parsed = parseVeld(veldSource);
      if (parsed.errors.length > 0) {
        setParseErrors(parsed.errors);
        setGenerated(null);
      } else if (parsed.models.length === 0 && parsed.modules.length === 0) {
        setParseErrors(['No models or modules found. Write a model or module to see generated output.']);
        setGenerated(null);
      } else {
        // Run semantic validation
        const validationErrors = validateParsed(parsed);
        if (validationErrors.length > 0) {
          setParseErrors(validationErrors);
          setGenerated(null);
        } else {
          setParseErrors([]);
          setGenerated({
            frontend: generateFrontendSDK(parsed),
            backend: generateBackendRoutes(parsed),
            sdkUsage: generateSDKUsage(parsed),
            backendImpl: generateBackendImpl(parsed),
          });
        }
      }
      setGenerating(false);
    }, 400);
  }, [veldSource]);

  const handlePreset = (key: string) => {
    setVeldSource(presets[key].code);
    setGenerated(null);
    setParseErrors([]);
  };

  const handleReset = () => {
    setVeldSource('');
    setGenerated(null);
    setParseErrors([]);
  };

  return (
    <section className={styles.section} id="try-it">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Try it yourself</h2>
          <p className={styles.subtitle}>
            Write a <code>.veld</code> contract below and hit <strong>Generate</strong> to see the
            typed frontend SDK and backend routes Veld produces.
          </p>
        </motion.div>

        {/* Preset buttons */}
        <div className={styles.presets}>
          <span className={styles.presetsLabel}>Load example:</span>
          {Object.entries(presets).map(([key, preset]) => (
            <button
              key={key}
              className={styles.presetBtn}
              onClick={() => handlePreset(key)}
            >
              {preset.label}
            </button>
          ))}
        </div>

        <motion.div
          className={styles.playground}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.1 }}
        >
          {/* ── Left: Editor ── */}
          <div className={styles.editorPane}>
            <div className={styles.paneHeader}>
              <div className={styles.paneHeaderLeft}>
                <span className={styles.dot} style={{ background: '#f85149' }} />
                <span className={styles.dot} style={{ background: '#f0883e' }} />
                <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
                <span className={styles.paneTitle}>
                  <FileCode size={14} />
                  contract.veld
                </span>
              </div>
              <button className={styles.resetBtn} onClick={handleReset} title="Clear editor">
                <RotateCcw size={14} />
              </button>
            </div>
            <div className={styles.editorWrap}>
              <pre ref={highlightRef} className={styles.editorHighlight} aria-hidden="true">
                {highlightVeld(veldSource + '\n')}
              </pre>
              <textarea
                ref={editorRef}
                className={styles.editor}
                value={veldSource}
                onChange={(e) => setVeldSource(e.target.value)}
                onScroll={syncScroll}
                spellCheck={false}
                placeholder={`// Write your .veld contract here\n\nmodel User {\n  id:    uuid\n  name:  string\n}\n\nmodule Users {\n  prefix: /api\n\n  action GetUser {\n    method: GET\n    path:   /users/:id\n    output: User\n  }\n}`}
              />
            </div>
          </div>

          {/* ── Center: Generate button ── */}
          <div className={styles.generateCol}>
            <button
              className={`${styles.generateBtn} ${generating ? styles.generating : ''}`}
              onClick={handleGenerate}
              disabled={generating || !veldSource.trim()}
            >
              <Play size={18} />
              <span>Generate</span>
            </button>
          </div>

          {/* ── Right: Output ── */}
          <div className={styles.outputPane}>
            <div className={styles.paneHeader}>
              <div className={styles.outputTabs}>
                <button
                  className={`${styles.outputTab} ${activeTab === 'frontend' ? styles.activeTab : ''}`}
                  onClick={() => setActiveTab('frontend')}
                >
                  <Monitor size={14} />
                  Frontend SDK
                </button>
                <button
                  className={`${styles.outputTab} ${activeTab === 'backend' ? styles.activeTab : ''}`}
                  onClick={() => setActiveTab('backend')}
                >
                  <Server size={14} />
                  Backend Routes
                </button>
                <button
                  className={`${styles.outputTab} ${activeTab === 'sdk-usage' ? styles.activeTab : ''}`}
                  onClick={() => setActiveTab('sdk-usage')}
                >
                  <Code size={14} />
                  SDK Usage
                </button>
                <button
                  className={`${styles.outputTab} ${activeTab === 'backend-impl' ? styles.activeTab : ''}`}
                  onClick={() => setActiveTab('backend-impl')}
                >
                  <Wrench size={14} />
                  Implement
                </button>
              </div>
            </div>

            <div className={styles.outputBody}>
              {parseErrors.length > 0 && (
                <div className={styles.errorBox}>
                  {parseErrors.map((err, i) => (
                    <div key={i} className={styles.errorLine}>⚠ {err}</div>
                  ))}
                </div>
              )}

              {!generated && parseErrors.length === 0 && (
                <div className={styles.placeholder}>
                  <Play size={32} />
                  <p>Write a contract and hit <strong>Generate</strong> to see output</p>
                </div>
              )}

              {generated && (
                <pre className={styles.outputCode}>
                  {highlightTS(
                    activeTab === 'frontend'
                      ? generated.frontend
                      : activeTab === 'backend'
                      ? generated.backend
                      : activeTab === 'sdk-usage'
                      ? generated.sdkUsage
                      : generated.backendImpl
                  )}
                </pre>
              )}
            </div>
          </div>
        </motion.div>
      </div>
    </section>
  );
}

