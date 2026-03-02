# php-typescript

Laravel backend + vanilla TypeScript fetch SDK example using Veld.

## What this example shows

- Laravel backend wired to Veld-generated controllers and route registrations
- In-memory service implementations for `Users` and `Todos`
- Raw TypeScript frontend usage of the Veld-generated `api` client (no framework)

## Project structure

```
php-typescript/
├── veld/                  ← Veld contracts (models + modules)
│   ├── veld.config.json
│   ├── app.veld
│   ├── models/
│   └── modules/
├── backend/               ← Laravel application
│   ├── app/
│   │   └── Services/
│   │       ├── UsersService.php
│   │       └── TodosService.php
│   ├── routes/
│   │   └── api.php        ← includes generated route file
│   └── composer.json
├── frontend/
│   └── example.ts         ← SDK usage examples
└── generated/             ← created by `veld generate`
    ├── app/
    │   ├── Models/        ← PHP readonly DTOs
    │   ├── Services/      ← I{Module}Service interfaces
    │   └── Http/Controllers/
    ├── routes/
    │   └── api.php        ← Laravel Route:: registrations
    └── client/
        └── api.ts         ← TypeScript fetch SDK
```

## Getting started

### 1. Generate the code

```bash
cd veld
veld generate
```

This creates `../generated/` with PHP models, interfaces, controllers, routes, and the TypeScript client SDK.

### 2. Install backend dependencies

```bash
cd backend
composer install
```

Requires PHP 8.2+ and Laravel 11.

### 3. Run the backend

```bash
php artisan serve
```

The server starts on `http://localhost:8000`.

### 4. Try the SDK (frontend/example.ts)

The `frontend/example.ts` file demonstrates direct usage of the generated `api` client.
Run it with `ts-node` or `tsx`:

```bash
npx tsx frontend/example.ts
```

## API routes

| Method | Path          | Description       |
|--------|---------------|-------------------|
| GET    | /users        | List all users    |
| GET    | /users/{id}   | Get user by ID    |
| POST   | /users        | Create a user     |
| DELETE | /users/{id}   | Delete a user     |
| GET    | /todos        | List all todos    |
| GET    | /todos/{id}   | Get todo by ID    |
| POST   | /todos        | Create a todo     |
| PUT    | /todos/{id}   | Update a todo     |
| DELETE | /todos/{id}   | Delete a todo     |
