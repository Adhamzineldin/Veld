# node-flutter

Express backend + Dart/Flutter client example powered by Veld.

## What this shows

- Node.js/Express backend wired to Veld-generated route handlers and Zod validation
- In-memory service implementations for `Users` and `Todos`
- Flutter `StatefulWidget` using the generated `VeldApi` Dart client to list and create todos

## Project structure

```
node-flutter/
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
│   └── todo_screen.dart   ← Flutter widget using the generated Dart client
└── generated/             ← created by `veld generate`
```

## Getting started

### 1. Generate the code

```bash
cd veld
veld generate
```

This writes typed interfaces, route handlers, Zod schemas, and the Dart client into `../generated/`.

### 2. Start the backend

```bash
cd backend
npm install
npm run dev
```

The server listens on `http://localhost:3000`.

### 3. Use the Flutter widget

Copy `frontend/todo_screen.dart` into your Flutter app's `lib/` folder.
The generated Dart client lives at `generated/client/api_client.dart` — add it to your
Flutter project (local path dependency in `pubspec.yaml`) or copy it into `lib/`.

```yaml
# pubspec.yaml (your Flutter app)
dependencies:
  flutter:
    sdk: flutter
  veld_client:
    path: ../generated/client
```

Then navigate to `TodoScreen` from your app's router.

## API routes

| Method | Path        | Description      |
|--------|-------------|------------------|
| GET    | /users      | List all users   |
| GET    | /users/:id  | Get user by ID   |
| POST   | /users      | Create a user    |
| DELETE | /users/:id  | Delete a user    |
| GET    | /todos      | List all todos   |
| GET    | /todos/:id  | Get todo by ID   |
| POST   | /todos      | Create a todo    |
| PUT    | /todos/:id  | Update a todo    |
| DELETE | /todos/:id  | Delete a todo    |
