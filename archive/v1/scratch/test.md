Title: Veld — Contract-First API Code Generator

URL Source: https://app2501.maayn.com/docs

Markdown Content:
Veld Documentation
------------------

Everything you need to know about writing `.veld` contracts and generating typed backends, frontend SDKs, validation, and more.

Overview
--------

**Veld** is a contract-first, multi-stack API code generator. You write`.veld` contract files describing your models, enums, and API endpoints. Veld then generates fully typed backend service interfaces, route wiring with input validation, frontend SDKs, OpenAPI specs, database schemas, and more — for any stack you choose.

**Zero runtime dependencies** — generated code works out of the box with no `npm install` needed (for type-only usage). Validation schemas (Zod/Pydantic) are opt-in.

*   Write your API contract once, generate code for 7+ backend languages and 4+ frontend targets
*   Framework agnostic — works with Express, Fastify, Hono, Flask, and any router with `.get()`/`.post()`
*   Deterministic output — same input always produces identical output, safe for CI/CD
*   Built-in validation with Zod (Node.js) and Pydantic (Python)
*   OpenAPI 3.0 spec generation from the same contract
*   Watch mode for instant re-generation on file save
*   IDE support with VS Code extension, JetBrains plugin, and built-in LSP server

Installation
------------

Veld is available on all major package managers. Pick whichever works best for your setup.

$ npm install -g @maayn/veld

# Verify installation:
$ veld --version
veld v0.1.0

# Run generation:
$ npx @maayn/veld generate

### System Requirements

*   **OS:** Windows, macOS, or Linux (amd64 & arm64)
*   **Runtime:** None required — Veld is a standalone binary compiled from Go
*   **Node.js:** Required only if using `npx` to run Veld or for Node.js backend output
*   **Python:** Required only for Python backend output

### Verify Installation

$ veld --version
veld v0.1.0

$ veld --help
Veld — Contract-first API code generator

Usage:
veld [command]

Available Commands:
init        Initialize a new Veld project
generate    Generate backend and frontend code
validate    Validate contract files
watch       Watch for changes and auto-regenerate
openapi     Export OpenAPI 3.0 spec
schema      Generate database schemas
docs        Generate API documentation
diff        Show contract differences
ast         Dump AST as JSON
clean       Remove generated output
lsp         Start LSP server
help        Help about any command

Quick Start
-----------

Get up and running with Veld in under 2 minutes.

### Step 1: Initialize a new project

$ mkdir my-api && cd my-api
$ veld init

✓ Created veld/veld.config.json
✓ Created veld/app.veld
✓ Created veld/models/
✓ Created veld/modules/

Done! Edit veld/app.veld to define your API contract.

### Step 2: Write your contract

model User {

id: uuid

email: string

name: string

role: Role @default(user)

}

model CreateUserInput {

email: string

name: string

}

enum Role { admin user guest }

module Users {

prefix: /api/v1

action ListUsers {

method: GET

path: /users

output: User[]

}

action GetUser {

method: GET

path: /users/:id

output: User

}

action CreateUser {

method: POST

path: /users

input: CreateUserInput

output: User

}

action DeleteUser {

method: DELETE

path: /users/:id

}

}

### Step 3: Generate code

$ veld generate

✓ Generated types/users.ts
✓ Generated interfaces/IUsersService.ts
✓ Generated routes/users.routes.ts
✓ Generated schemas/schemas.ts
✓ Generated client/api.ts
✓ Generated index.ts
✓ Generated package.json

Done! 7 files generated in generated/

### Step 4: Implement your service

import { IUsersService } from '@veld/generated/interfaces/IUsersService';

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

}

### Step 5: Wire it up

import express from 'express';

import { registerUsersRoutes } from '@veld/generated/routes/users.routes';

import { UsersService } from './services/UsersService';

const app = express();

app.use(express.json());

const usersService = new UsersService();

registerUsersRoutes(app, usersService);

app.listen(3000, () => {

console.log('Server running on http://localhost:3000');

});

### Step 6: Use the frontend SDK

import { Users } from '@veld/generated/client/api';

// Fully typed — autocomplete for methods, params, and return types

const users = await Users.listUsers();

// ^? Promise<User[]>

const user = await Users.getUser('user-123');

// ^? Promise<User>

await Users.createUser({

email: 'alice@example.com',

name: 'Alice',

});

// ^? Promise<User>

Project Structure
-----------------

After running `veld init`, your project has this structure:

my-project/ ├── veld/ <- all veld source files │ ├── veld.config.json <- configuration │ ├── app.veld <- entry point contract │ ├── models/ <- model definitions │ └── modules/ <- module/action definitions └── generated/ <- auto-generated on first `veld generate` ├── index.ts <- barrel export ├── package.json <- @veld/generated alias ├── types/ <- TypeScript interfaces ├── interfaces/ <- service contracts ├── routes/ <- route handlers ├── schemas/ <- validation schemas └── client/ <- frontend SDK

**Note:** Veld never writes outside the `--out` directory (defaults to `generated/`). Your source code is never touched. The generated directory is safe to delete and regenerate at any time.

Models
------

Models define data structures in your API. Each model becomes a TypeScript interface, a Zod schema, and corresponding types in your chosen backend language.

model User {

description: "A registered user in the system"

id: uuid

email: string

name: string

age?: int // optional field

tags: string[] // array type

metadata: Map<string, string> // map/record type

role: Role @default(user) // default value

createdAt: datetime

}

### Model Syntax

| Syntax | Meaning | Example |
| --- | --- | --- |
| `fieldName: type` | Required field | `email: string` |
| `fieldName?: type` | Optional field | `bio?: string` |
| `field: type[]` | Array of values | `tags: string[]` |
| `field: Map<K, V>` | Key-value map | `metadata: Map<string, string>` |
| `@default(value)` | Default value decorator | `role: Role @default(user)` |
| `description: "..."` | Model description (for docs/OpenAPI) | `description: "A user account"` |

Enums
-----

Enums define a set of named constants. They generate TypeScript union types, Zod enums, and Python string enums.

// Single-line enum

enum Role { admin user guest }

// Multi-line enum

enum OrderStatus {

pending

confirmed

shipped

delivered

cancelled

}

### Generated Output

export type Role = 'admin' | 'user' | 'guest';

export type OrderStatus = 'pending' | 'confirmed' | 'shipped' | 'delivered' | 'cancelled';

export const RoleSchema = z.enum(['admin', 'user', 'guest']);

export const OrderStatusSchema = z.enum(['pending', 'confirmed', 'shipped', 'delivered', 'cancelled']);

Modules & Actions
-----------------

Modules group related API endpoints. Each module has a prefix and contains actions that define individual HTTP endpoints.

module Users {

description: "User management endpoints"

prefix: /api/v1

action ListUsers {

description: "List all users with pagination"

method: GET

path: /users

query: ListUsersQuery

output: User[]

}

action GetUser {

description: "Get a user by ID"

method: GET

path: /users/:id

output: User

}

action CreateUser {

description: "Create a new user"

method: POST

path: /users

input: CreateUserInput

output: User

middleware: AuthGuard

}

action UpdateUser {

method: PUT

path: /users/:id

input: UpdateUserInput

output: User

}

action DeleteUser {

method: DELETE

path: /users/:id

}

}

### Action Fields

| Field | Required | Description |
| --- | --- | --- |
| `method` | Yes | HTTP method: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, or `WS` |
| `path` | Yes | URL path, supports params like `/users/:id` |
| `input` | No | Request body model name. Generates Zod validation. |
| `output` | No | Response body model/type. `User[]` for arrays. |
| `query` | No | Query parameters model name |
| `errors` | No | Error codes: `[NotFound, Forbidden]` |
| `middleware` | No | Middleware name (e.g. `AuthGuard`) |
| `description` | No | Action description (used in OpenAPI/docs) |
| `stream` | No | WebSocket message type (requires `method: WS`) |

### HTTP Status Codes

Veld automatically generates the correct HTTP status codes:

| Method | With Output | Without Output |
| --- | --- | --- |
| `GET` | 200 OK | 200 OK |
| `POST` | 201 Created | 201 Created |
| `PUT` | 200 OK | 200 OK |
| `PATCH` | 200 OK | 200 OK |
| `DELETE` | 200 OK | 204 No Content |

Field Types
-----------

Veld supports a set of built-in primitive types that map to the appropriate types in each target language and validation library.

| Veld | TypeScript | Python | Zod | Pydantic |
| --- | --- | --- | --- | --- |
| `string` | `string` | `str` | `z.string()` | `str` |
| `int` | `number` | `int` | `z.number().int()` | `int` |
| `float` | `number` | `float` | `z.number()` | `float` |
| `bool` | `boolean` | `bool` | `z.boolean()` | `bool` |
| `date` | `string` | `str` | `z.string().date()` | `str` |
| `datetime` | `string` | `str` | `z.string().datetime()` | `str` |
| `uuid` | `string` | `str` | `z.string().uuid()` | `str` |
| `T[]` | `T[]` | `List[T]` | `z.array(TSchema)` | `List[T]` |
| `Map<string, V>` | `Record<string, V>` | `Dict[str, V]` | `z.record(z.string(), V)` | `Dict[str, V]` |

**Custom types:** Any `PascalCase` name not in the built-in types is treated as a reference to a model or enum defined elsewhere in your contract.

Inheritance (extends)
---------------------

Models can extend other models to inherit all their fields. This generates TypeScript `interface X extends Y`, Zod `.extend()`, and Python class inheritance.

model BaseEntity {

id: uuid

createdAt: datetime

updatedAt: datetime

}

model User extends BaseEntity {

email: string

name: string

role: Role

}

model Admin extends User {

permissions: string[]

}

### Generated TypeScript

export interface BaseEntity {

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

}

**Circular inheritance** is detected and rejected by the validator. For example, `A extends B` and `B extends A` will produce a clear error with file and line numbers.

Maps
----

Use `Map<K, V>` syntax to define key-value pair fields. Maps generate`Record<string, V>` in TypeScript and `Dict[str, V]` in Python.

model Config {

settings: Map<string, string>

features: Map<string, bool>

metadata: Map<string, int>

}

**Note:** Map keys are always `string`. The value type can be any built-in type or a reference to a model/enum.

Import System
-------------

Veld supports two import styles for organizing contracts across multiple files.

### Alias-based imports (recommended)

import @models/user

import @models/product

import @modules/users

import @modules/shop

Aliases are resolved from the project root using the `aliases` config. Default aliases include: `@models`, `@modules`, `@types`,`@enums`, `@schemas`, `@services`, `@lib`,`@common`, `@shared`.

### Relative imports (legacy)

import "./models/user.veld"

import "./modules/users.veld"

**Both styles** are fully supported in the CLI, VS Code extension, and JetBrains plugin. Alias imports are preferred for cleaner, more portable contracts.

WebSockets
----------

Veld supports WebSocket actions with the `WS` method and`stream` field for typed message payloads.

model ChatMessage {

userId: uuid

content: string

sentAt: datetime

}

module Chat {

prefix: /ws

action ChatStream {

method: WS

path: /chat/:roomId

stream: ChatMessage

}

}

WebSocket actions generate typed connect methods in the frontend SDK and comment stubs in the backend route handlers. The `stream` field specifies the message type that flows through the WebSocket connection.

**Validation:** The `stream` field is only valid on`method: WS` actions. WS actions require a `stream` type. Using `input`/`output` on WS actions is not allowed.

CLI Overview
------------

The Veld CLI is a single binary with subcommands for every stage of the workflow.

$ veld --help

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
-v, --version   version for veld

veld init
---------

Scaffolds a new Veld project in the current directory. Creates the `veld/` folder with a config file, entry point, and subdirectories for models and modules.

$ veld init

✓ Created veld/veld.config.json
✓ Created veld/app.veld
✓ Created veld/models/
✓ Created veld/modules/

**Safety:**`veld init` exits with code 1 if the `veld/`directory already exists — it will never overwrite existing files.

veld generate
-------------

The main command. Reads your contract, validates it, and generates all output files.

# Use config auto-detection (reads veld.config.json):
$ veld generate

# Specify all options explicitly:
$ veld generate \
--backend=node \
--frontend=typescript \
--input=veld/app.veld \
--out=./generated

# Preview without writing files:
$ veld generate --dry-run

### Flags

| Flag | Default | Description |
| --- | --- | --- |
| `--backend` | from config | Backend emitter: `node`, `python`, `go`, `java`, `csharp`, `php`, `rust` |
| `--frontend` | from config | Frontend emitter: `typescript`, `dart`, `kotlin`, `swift`, `none` |
| `--input` | from config | Entry `.veld` file path |
| `--out` | from config | Output directory |
| `--dry-run` | `false` | Preview generated files without writing |

veld validate
-------------

Validates your contract without generating any output. Reports errors with file names, line numbers, and source code snippets.

$ veld validate

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
Error: Input type "NonExistent" is not defined

veld watch
----------

Watches your `.veld` files for changes and auto-regenerates with a 500ms debounce. Perfect for development.

$ veld watch

Watching for changes in veld/ ...

[12:00:01] Changed: veld/models/user.veld
[12:00:01] Regenerating...
[12:00:01] ✓ Done (7 files in 42ms)

veld openapi
------------

Exports an OpenAPI 3.0 specification from your contract. Output to stdout or a file.

# Print to stdout:
$ veld openapi

# Write to file:
$ veld openapi -o openapi.json

# Pipe to another tool:
$ veld openapi | jq '.paths'

veld schema
-----------

Generates database schemas from your models. Supports Prisma schema format and raw SQL DDL.

# Generate Prisma schema:
$ veld schema --format=prisma -o schema.prisma

# Generate SQL DDL:
$ veld schema --format=sql -o schema.sql

veld docs
---------

Generates human-readable API documentation from your contract. Useful for teams and stakeholders who don't read code.

$ veld docs -o api-docs.md

veld diff
---------

Shows the differences between contract versions. Detects added/removed/changed models, fields, actions, and types.

$ veld diff --old=v1/app.veld --new=v2/app.veld

+ Added model: PaymentMethod
  ~ Changed model: User
    + Added field: avatarUrl (string)
    - Removed field: profilePic
      ~ Changed action: CreateUser
      ~ input changed: CreateUserInput -> CreateUserInputV2

veld ast
--------

Dumps the parsed AST as JSON. Useful for debugging, tooling, or building custom code generators on top of Veld's parser.

$ veld ast | jq '.models[0]'
{
"name": "User",
"fields": [
{ "name": "id", "type": "uuid", "optional": false },
{ "name": "email", "type": "string", "optional": false }
]
}

veld clean
----------

Removes the generated output directory. A clean slate for regeneration.

$ veld clean

✓ Removed generated/

Configuration File
------------------

Veld uses a JSON configuration file to set defaults for all CLI commands. This file is created automatically by `veld init`.

{
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
}

Config Fields
-------------

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `input` | string | _required_ | Entry `.veld` file (relative to config location) |
| `backend` | string | `"node"` | Backend emitter: `node`, `python`, `go`, `java`, `csharp`, `php`, `rust` |
| `frontend` | string | `"typescript"` | Frontend emitter: `typescript`, `react` (alias), `dart`, `flutter` (alias), `kotlin`, `swift`, `none` |
| `out` | string | `"./generated"` | Output directory (relative to config location) |
| `baseUrl` | string | `""` | Baked into frontend SDK. If empty, uses `process.env.VELD_API_URL` |
| `aliases` | object | built-in defaults | Custom `@alias` to relative directory mappings |

Import Aliases
--------------

Aliases map short `@name` prefixes to directories, relative to the config file.

{
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
}

With this config, `import @models/user` resolves to `veld/models/user.veld`. You can add custom aliases for any directory structure:

{
"aliases": {
"auth": "services/auth",
"payments": "features/payments/contracts"
}
}

Config Auto-Detection
---------------------

When you run `veld generate` (no flags), Veld searches for the config file in this order:

1.   `./veld.config.json` in the current directory
2.   `./veld/veld.config.json` in the `veld/` subdirectory

CLI flags always override config file values. For example:

# Config says backend=node, but override with python:
$ veld generate --backend=python

Generated Output Overview
-------------------------

Veld generates a complete set of files organized by purpose. All generated files begin with `// AUTO-GENERATED BY VELD — DO NOT EDIT`.

**Key principles:** Output is deterministic (same input = same output), Veld never writes outside `--out`, and generated code has zero runtime dependencies for type-only usage.

Node.js Backend Output
----------------------

generated/ ├── index.ts # Barrel export for clean imports ├── package.json # @veld/generated package alias ├── types/ │ ├── users.ts # Types owned by Users module │ ├── auth.ts # Types owned by Auth module + re-exports shared │ └── index.ts # Barrel re-export of all module type files ├── interfaces/ │ └── IUsersService.ts # Service contract interface (typed path params) ├── routes/ │ └── users.routes.ts # Route handlers: try/catch, Zod validation, HTTP status codes ├── schemas/ │ └── schemas.ts # Zod validation schemas (supports extends) └── client/ └── api.ts # Frontend SDK with VeldApiError, path params

Types are emitted into per-module files. Each type is **defined** in exactly one file (the first module to use it). Other modules re-export shared types. A barrel `types/index.ts` re-exports everything.

Python Backend Output
---------------------

generated/ ├── __init__.py ├── types/ │ ├── users.py # Types owned by Users module │ ├── auth.py # Types owned by Auth module + re-imports shared │ └── __init__.py # Barrel re-import ├── interfaces/ │ └── i_users_service.py # ABC service contract ├── routes/ │ └── users_routes.py # Flask handlers: try/except, Pydantic validation └── schemas/ └── schemas.py # Pydantic BaseModel schemas

Frontend SDK
------------

The generated frontend SDK uses native `fetch` (no axios dependency) with full type safety.

### Features

*   **VeldApiError** class with `status` and `body` fields for type-safe error handling
*   **Path parameter interpolation:**`/users/:id` becomes `/users/${id}` with typed `id: string` param
*   **All HTTP methods:**`get()`, `post()`, `put()`, `patch()`, `del()`
*   **Base URL:** configurable via config or `process.env.VELD_API_URL`
*   **Zero dependencies:** uses only native `fetch`

// AUTO-GENERATED BY VELD — DO NOT EDIT

export class VeldApiError extends Error {

constructor(public status: number, public body: unknown) {

super(`API error ${status}`);

}

}

const BASE_URL = process.env.VELD_API_URL || '';

export const Users = {

async listUsers(): Promise<User[]> {

const res = await fetch(`${BASE_URL}/api/v1/users`);

if (!res.ok) throw new VeldApiError(res.status, await res.json());

return res.json();

},

async getUser(id: string): Promise<User> {

const res = await fetch(`${BASE_URL}/api/v1/users/${id}`);

if (!res.ok) throw new VeldApiError(res.status, await res.json());

return res.json();

},

async createUser(data: CreateUserInput): Promise<User> {

const res = await fetch(`${BASE_URL}/api/v1/users`, {

method: 'POST',

headers: { 'Content-Type': 'application/json' },

body: JSON.stringify(data),

});

if (!res.ok) throw new VeldApiError(res.status, await res.json());

return res.json();

},

};

Validation Schemas
------------------

Veld generates validation schemas that are used automatically in route handlers.

### Node.js (Zod)

import { z } from 'zod';

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

});

### Python (Pydantic)

from pydantic import BaseModel
from typing import Optional

class UserSchema(BaseModel):
id: str
email: str
name: str
role: str = 'user'

class CreateUserInputSchema(BaseModel):
email: str
name: str

Route Handlers
--------------

Generated route handlers include try/catch wrapping, input validation, correct HTTP status codes, and path parameter extraction.

import { IUsersService } from '../interfaces/IUsersService';

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

}

**Framework agnostic:** The `router` parameter accepts`any` — wire in Express, Fastify, Hono, or any router with`.get()`/`.post()` methods.

Backend Integration
-------------------

Implement the generated service interface, then wire it to your router. Veld never touches your business logic files.

### 1. Implement the interface

import { IUsersService } from '@veld/generated/interfaces/IUsersService';

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

}

### 2. Wire routes to your server

import express from 'express';

import { registerUsersRoutes } from '@veld/generated/routes/users.routes';

import { UsersService } from './services/UsersService';

const app = express();

app.use(express.json());

registerUsersRoutes(app, new UsersService());

app.listen(3000);

### Works with any router

import Fastify from 'fastify';

import { registerUsersRoutes } from '@veld/generated/routes/users.routes';

import { UsersService } from './services/UsersService';

const app = Fastify();

registerUsersRoutes(app, new UsersService());

app.listen({ port: 3000 });

Frontend SDK Usage
------------------

The generated frontend SDK provides fully typed methods for every action in your contract.

import { Users } from '@veld/generated/client/api';

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

console.error(err.body); // { error: '...' }

}

}

### Setting the Base URL

// Option 1: Environment variable

// Set VELD_API_URL=https://api.example.com in your .env

// Option 2: Config file — set baseUrl in veld.config.json

// { "baseUrl": "https://api.example.com" }

// The SDK reads from:

// 1. baseUrl from config (baked into generated code)

// 2. process.env.VELD_API_URL (runtime fallback)

Path Aliases
------------

The generated `package.json` enables the `@veld/generated` import alias. Add this to your `tsconfig.json` to use it:

{
"compilerOptions": {
"paths": {
"@veld/*": ["./generated/*"]
}
}
}

Then import generated code anywhere:

import { User } from '@veld/generated/types';

import { IUsersService } from '@veld/generated/interfaces/IUsersService';

import { registerUsersRoutes } from '@veld/generated/routes/users.routes';

import { Users } from '@veld/generated/client/api';

Backend Emitters
----------------

Veld supports 7 backend target languages. Each emitter generates types, service interfaces, route handlers, and validation schemas in the target language.

| Flag Value | Language | Validation | Route Style |
| --- | --- | --- | --- |
| `node` | TypeScript (Node.js) | Zod | Express/Fastify/Hono |
| `python` | Python | Pydantic | Flask |
| `go` | Go | Built-in | Chi/Mux |
| `java` | Java | Jakarta | Spring Boot |
| `csharp` | C# | DataAnnotations | ASP.NET |
| `php` | PHP | Built-in | Laravel |
| `rust` | Rust | serde | Actix/Axum |

Frontend Emitters
-----------------

| Flag Value | Language | Output File | Aliases |
| --- | --- | --- | --- |
| `typescript` | TypeScript | `client/api.ts` | `react` |
| `dart` | Dart | `client/api_client.dart` | `flutter` |
| `kotlin` | Kotlin | `client/ApiClient.kt` | — |
| `swift` | Swift | `client/APIClient.swift` | — |
| `none` | — | No frontend SDK generated | — |

VS Code Extension
-----------------

The Veld VS Code extension provides a first-class editing experience for `.veld` files.

### Installation

1.   Open VS Code
2.   Go to Extensions (`Ctrl+Shift+X` / `Cmd+Shift+X`)
3.   Search for **"Veld"**
4.   Click Install

### Features

*   Syntax highlighting for `.veld` files
*   Real-time diagnostics (errors and warnings)
*   Autocomplete for keywords, types, and model/enum references
*   Hover information for types and actions
*   Go-to-definition for model and enum references
*   Code snippets for models, modules, and actions

JetBrains Plugin
----------------

Available for IntelliJ IDEA, WebStorm, PyCharm, GoLand, and all JetBrains IDEs.

### Installation

1.   Open Settings / Preferences
2.   Go to Plugins → Marketplace
3.   Search for **"Veld"**
4.   Click Install and restart the IDE

### Features

*   Syntax highlighting and code folding
*   Error highlighting and quick-fixes
*   Autocomplete for all Veld keywords and types
*   Navigate to definition

LSP Server
----------

Veld includes a built-in Language Server Protocol (LSP) server that any editor can use for diagnostics, completions, and hover information.

$ veld lsp

# The LSP server communicates via JSON-RPC 2.0 over stdin/stdout.
# Configure your editor to launch "veld lsp" as the language server
# for .veld files.

### Supported LSP Features

*   **textDocument/publishDiagnostics** — real-time error reporting
*   **textDocument/completion** — keyword, type, and reference completions
*   **textDocument/hover** — type information on hover
*   **textDocument/definition** — go-to-definition for model/enum references

### Neovim Setup

vim.api.nvim_create_autocmd('FileType', {
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
})