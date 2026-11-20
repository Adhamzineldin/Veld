# node-typescript

Express backend + vanilla TypeScript fetch SDK example using Veld.

## What this example shows

- Node.js/Express backend wired to Veld-generated route handlers
- In-memory service implementations for `Users` and `Todos`
- Raw TypeScript frontend usage of the Veld-generated `api` client (no framework)

## Project structure

```
node-typescript/
├── veld/                  ← Veld contracts (models + modules)
│   ├── veld.config.json
│   ├── app.veld
│   ├── models/
│   └── modules/
├── backend/               ← Express server
│   ├── src/
│   │   ├── index.ts
│   │   └── services/
│   │       ├── UsersService.ts
│   │       └── TodosService.ts
│   ├── package.json
│   └── tsconfig.json
├── frontend/
│   └── example.ts         ← SDK usage examples
└── generated/             ← created by `veld generate`
```

## Getting started

### 1. Generate the code

```bash
cd veld
veld generate
```

This creates `../generated/` with types, interfaces, routes, schemas, and the client SDK.

### 2. Install backend dependencies

```bash
cd backend
npm install
```

### 3. Run the backend

```bash
npm run dev
```

The server starts on `http://localhost:3000`.

### 4. Try the SDK (frontend/example.ts)

The `frontend/example.ts` file demonstrates direct usage of the generated `api` client.
Run it with `ts-node` or `tsx`:

```bash
npx tsx ../frontend/example.ts
```

## API routes

| Method | Path          | Description       |
|--------|---------------|-------------------|
| GET    | /users        | List all users    |
| GET    | /users/:id    | Get user by ID    |
| POST   | /users        | Create a user     |
| DELETE | /users/:id    | Delete a user     |
| GET    | /todos        | List all todos    |
| GET    | /todos/:id    | Get todo by ID    |
| POST   | /todos        | Create a todo     |
| PUT    | /todos/:id    | Update a todo     |
| DELETE | /todos/:id    | Delete a todo     |
