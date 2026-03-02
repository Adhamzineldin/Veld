# Rust + Swift (iOS) Example Setup Guide

## Quick Start

```bash
# Backend (Rust/Axum)
cd rust-swift/backend && cargo run
# Runs on http://localhost:3000

# Frontend (Swift/SwiftUI)
cd rust-swift/frontend
# Open TodoApp.xcodeproj in Xcode and run
```

## Backend Setup (Rust / Axum)

### Prerequisites
- Rust 1.70+
- Cargo

### Installation

```bash
cd backend

# Download dependencies
cargo build

# Run development server
cargo run
# Runs on http://localhost:3000

# Run with watch
cargo watch -x run
```

### Project Structure

```
backend/
├── src/
│   ├── main.rs
│   ├── services/
│   │   ├── todos.rs
│   │   └── users.rs
│   └── models/
│       ├── todo.rs
│       └── user.rs
├── Cargo.toml
└── Cargo.lock
```

### Key Rust/Axum Features

**Web Framework:**
```rust
use axum::{
    routing::{get, post, put, delete},
    Router, Json,
};

let app = Router::new()
    .route("/todos", post(create_todo))
    .route("/todos/:id", get(get_todo).put(update_todo).delete(delete_todo))
    .route("/todos", get(list_todos));
```

**Async/Await Service:**
```rust
pub struct TodosService {
    todos: Arc<Mutex<Vec<Todo>>>,
}

impl TodosService {
    pub async fn list_todos(&self) -> Result<Vec<Todo>, String> {
        let todos = self.todos.lock().unwrap();
        Ok(todos.clone())
    }
    
    pub async fn create_todo(&self, input: CreateTodoInput) -> Result<Todo, String> {
        let todo = Todo {
            id: Uuid::new_v4().to_string(),
            title: input.title,
            completed: false,
            user_id: input.user_id,
        };
        self.todos.lock().unwrap().push(todo.clone());
        Ok(todo)
    }
}
```

**Error Handling:**
```rust
use axum::http::StatusCode;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    InvalidInput(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
        };
        (status, message).into_response()
    }
}
```

**Type Safety:**
```rust
// Compile-time type checking
pub struct Todo {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub user_id: String,
}

impl Serialize for Todo { }
impl Deserialize for Todo { }

// Returns are enforced at compile time
pub async fn get_todo(id: String) -> Result<Json<Todo>, AppError> { }
```

## Frontend Setup (Swift / SwiftUI)

### Prerequisites
- Xcode 14.3+
- iOS 14.0+

### Installation

```bash
cd frontend

# Install dependencies (CocoaPods)
pod install

# Open in Xcode
open TodoApp.xcworkspace

# Run on simulator or device
# Xcode → Product → Run (⌘R)
```

### Project Structure

```
TodoApp/
├── TodoApp.swift                # App entry point
├── Views/
│   ├── ContentView.swift
│   ├── TodoListView.swift
│   └── AddTodoView.swift
├── ViewModels/
│   ├── TodoViewModel.swift
│   └── UserViewModel.swift
├── Models/
│   ├── Todo.swift
│   └── User.swift
├── Services/
│   └── APIService.swift
└── generated/
    └── VeldApi.swift           # Generated client
```

### Key Swift/SwiftUI Features

**View with State:**
```swift
struct TodoView: View {
    @State private var todos: [Todo] = []
    @State private var isLoading = true
    @State private var errorMessage: String?
    
    var body: some View {
        NavigationStack {
            if isLoading {
                ProgressView()
            } else if let error = errorMessage {
                Text("Error: \(error)")
                    .foregroundColor(.red)
            } else {
                List {
                    ForEach(todos) { todo in
                        TodoRowView(todo: todo)
                    }
                }
            }
        }
        .onAppear {
            Task {
                await loadTodos()
            }
        }
    }
    
    @MainActor
    private func loadTodos() async {
        defer { isLoading = false }
        do {
            todos = try await VeldApi.Todos.listTodos()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
```

**ViewModel Pattern:**
```swift
@MainActor
class TodoViewModel: ObservableObject {
    @Published var todos: [Todo] = []
    @Published var isLoading = false
    @Published var errorMessage: String?
    
    func loadTodos() async {
        isLoading = true
        defer { isLoading = false }
        
        do {
            todos = try await VeldApi.Todos.listTodos()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }
    
    func addTodo(_ input: CreateTodoInput) async {
        do {
            let todo = try await VeldApi.Todos.createTodo(input: input)
            todos.append(todo)
        } catch {
            errorMessage = "Failed to create todo"
        }
    }
}
```

**Async/Await:**
```swift
// Modern Swift concurrency
async func getTodo(id: String) async throws -> Todo {
    return try await VeldApi.Todos.getTodo(id: id)
}

// Usage
Task {
    do {
        let todo = try await getTodo(id: "123")
        print(todo.title)
    } catch {
        print("Error: \(error)")
    }
}
```

**View Composition:**
```swift
struct TodoListView: View {
    let todos: [Todo]
    
    var body: some View {
        List {
            ForEach(todos) { todo in
                HStack {
                    Image(systemName: todo.completed ? "checkmark.circle.fill" : "circle")
                        .foregroundColor(todo.completed ? .green : .gray)
                    
                    Text(todo.title)
                        .strikethrough(todo.completed)
                    
                    Spacer()
                    
                    Button(action: { /* delete */ }) {
                        Image(systemName: "trash.fill")
                            .foregroundColor(.red)
                    }
                }
            }
        }
    }
}
```

## Architecture

### Rust Backend Structure
```
Request → Router → Handler
    ↓
Service (Business Logic)
    ↓
Models (Data Structures)
    ↓
In-Memory Store (or Database)
```

### Swift Frontend Structure
```
View (SwiftUI)
    ↓
ViewModel (State Management)
    ↓
API Service
    ↓
Generated VeldApi Client
    ↓
HTTP Request to Backend
```

## Running Together

### Terminal - Backend
```bash
cd rust-swift/backend
cargo run
# localhost:3000
```

### Xcode - Frontend
1. Open `TodoApp.xcworkspace` in Xcode
2. Select iOS Simulator or Physical Device
3. Product → Run (⌘R)
4. Configure API URL to `localhost:3000`

The app will connect and sync in real-time!

## Type Safety

### Rust Type System
```rust
// Compile-time verification
let todo: Todo = Todo {
    id: "123".to_string(),
    title: "Buy milk".to_string(),
    completed: false,
    user_id: "1".to_string(),
};

// Memory safety without garbage collector
// Thread safety without locks
// Type safety at compile time
```

### Swift Type Safety
```swift
// Generated types from Veld
let todo: Todo = try await VeldApi.Todos.getTodo(id: "123")

// Type checking
let todos: [Todo] = try await VeldApi.Todos.listTodos()

// Compile-time verification of required fields
let input = CreateTodoInput(title: "Buy milk", userId: "1")
```

## Production

### Backend (Rust)

Build optimized binary:
```bash
cargo build --release
# Creates binary in target/release/

./target/release/veld-rust-example
```

Deploy to:
- Fly.io
- Railway
- Heroku (with buildpack)
- AWS Lambda (with custom runtime)
- Any Linux/Unix server

### Frontend (Swift/iOS)

Build for App Store:
```bash
# In Xcode:
# Product → Archive
# Use Organizer to upload to App Store Connect
```

Or build for TestFlight:
```bash
# Xcode build settings
# CONFIGURATION=Release
# Product → Build For → Any iOS Device
```

## Performance

### Rust Advantages
- **Memory Safety** - No buffer overflows
- **Concurrency** - Lightweight async/await
- **Performance** - As fast as C/C++
- **Error Handling** - Explicit error propagation
- **Zero-Cost Abstractions** - No runtime overhead

### Swift Advantages
- **Native iOS Performance** - Direct hardware access
- **Seamless Integration** - Direct iOS/macOS APIs
- **Memory Safety** - Automatic memory management
- **Compile-Time Safety** - Swift type system
- **Easy Deployment** - App Store distribution

## Concurrency Model

### Rust - Tokio Runtime
```rust
// Multi-threaded async runtime
#[tokio::main]
async fn main() {
    let todos = list_todos().await;  // Non-blocking
}

// Handles thousands of concurrent connections
```

### Swift - DispatchQueue & Tasks
```swift
// GCD and async/await
Task {
    let todos = try await loadTodos()  // Non-blocking
    DispatchQueue.main.async {
        self.todos = todos
    }
}
```

## Troubleshooting

### Rust Issues
```bash
# Update Rust
rustup update

# Clean cache
cargo clean

# Check compilation
cargo check

# Run tests
cargo test
```

### Swift/Xcode Issues
```bash
# Clean build
Cmd + Shift + K

# Clear derived data
rm -rf ~/Library/Developer/Xcode/DerivedData/*

# Update pods
pod repo update
pod install

# Check SDK
xcode-select --install
```

### Connection Issues
- Ensure backend running on localhost:3000
- Check API URL in SwiftUI code
- On iOS: Can only connect to http if configured
- On Simulator: Use `127.0.0.1` or local IP

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Axum Web Framework](https://github.com/tokio-rs/axum)
- [Swift Language Guide](https://docs.swift.org/swift-book/)
- [SwiftUI Tutorial](https://developer.apple.com/tutorials/swiftui/)
- [iOS Development](https://developer.apple.com/ios/)
- [Veld Documentation](https://veld.dev)

---

Perfect for building **high-performance, memory-safe backends** with **beautiful native iOS apps**! 🦀📱

