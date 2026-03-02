# rust-swift

Axum backend + Swift iOS client example using Veld.

## What this example shows

- Rust/Axum backend wired to Veld-generated route handlers and service traits
- In-memory service implementations for `Users` and `Todos` using `Arc<Mutex<Vec<T>>>`
- SwiftUI iOS frontend using the Veld-generated `VeldApi` client

## Project structure

```
rust-swift/
├── veld/                  ← Veld contracts (models + modules)
│   ├── veld.config.json
│   ├── app.veld
│   ├── models/
│   └── modules/
├── backend/               ← Axum server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── services/
│           ├── mod.rs
│           ├── users.rs
│           └── todos.rs
├── frontend/
│   └── TodoView.swift     ← SwiftUI view using the generated API client
└── generated/             ← created by `veld generate`
```

## Getting started

### 1. Generate the code

```bash
cd veld
veld generate
```

This creates `../generated/` with `src/models.rs`, `src/services.rs`, per-module
route handlers, and `client/APIClient.swift`.

### 2. Run the backend

```bash
cd backend
cargo run
```

The server starts on `http://localhost:3000`.

### 3. Use the Swift client

Copy or reference `generated/client/APIClient.swift` and `frontend/TodoView.swift`
into your Xcode project. Point `VeldApi.baseURL` at your running server.

## API routes

| Method | Path        | Description     |
|--------|-------------|-----------------|
| GET    | /users      | List all users  |
| GET    | /users/:id  | Get user by ID  |
| POST   | /users      | Create a user   |
| DELETE | /users/:id  | Delete a user   |
| GET    | /todos      | List all todos  |
| GET    | /todos/:id  | Get todo by ID  |
| POST   | /todos      | Create a todo   |
| PUT    | /todos/:id  | Update a todo   |
| DELETE | /todos/:id  | Delete a todo   |
