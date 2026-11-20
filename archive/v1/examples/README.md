# Veld Examples

Reference examples showing how to wire Veld-generated code with real backends and frontends.

Each example uses the same `.veld` contract (a simple Todo + Users API) but targets a different backend + frontend combination.

## Examples

| Example | Backend | Frontend | Stack |
|---------|---------|----------|-------|
| [node-react](./node-react/) | Node (Express) | React (TanStack Query) | TypeScript full-stack |
| [node-typescript](./node-typescript/) | Node (Express) | TypeScript (fetch SDK) | TypeScript full-stack |
| [python-vue](./python-vue/) | Python (Flask) | Vue 3 (composables) | Python + TypeScript |
| [go-svelte](./go-svelte/) | Go (Chi) | Svelte 5 (stores) | Go + TypeScript |
| [java-angular](./java-angular/) | Java (Spring Boot) | Angular (services) | Java + TypeScript |
| [node-flutter](./node-flutter/) | Node (Express) | Dart (Flutter) | TypeScript + Dart |
| [rust-swift](./rust-swift/) | Rust (Axum) | Swift (iOS) | Rust + Swift |
| [csharp-kotlin](./csharp-kotlin/) | C# (ASP.NET) | Kotlin (Android) | C# + Kotlin |
| [php-typescript](./php-typescript/) | PHP (Laravel) | TypeScript (fetch SDK) | PHP + TypeScript |

## Contract

All examples share the same Veld contract defining a simple Todo + Users API:

- **Models**: `User`, `CreateUserInput`, `UpdateUserInput`, `Todo`, `CreateTodoInput`, `UpdateTodoInput`
- **Modules**: `Users` (List, Get, Create, Delete), `Todos` (List, Get, Create, Update, Delete)

## Quick Start

1. Build the Veld CLI (from repo root):
   ```bash
   go build -o veld ./cmd/veld
   ```

2. Pick an example and generate the code:
   ```bash
   cd examples/node-react
   ../../veld generate
   ```

3. Follow the example's README for setup and running instructions.

## What Each Example Shows

- **Backend**: How to implement the generated service interfaces and register routes
- **Frontend**: How to use the generated client SDK in framework-specific patterns
- **Veld config**: The `veld.config.json` for each backend/frontend combination
