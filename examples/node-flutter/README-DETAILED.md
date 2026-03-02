# Node.js + Flutter Example Setup Guide

## Quick Start

```bash
# Backend
cd node-flutter/backend && npm install && npm run dev
# Runs on http://localhost:3000

# Frontend (iOS/Android)
cd node-flutter/frontend && flutter pub get && flutter run
# Connects to localhost:3000
```

## Backend Setup (Node.js / Express)

### Prerequisites
- Node.js 16+
- npm or yarn

### Installation

```bash
cd backend
npm install
npm run dev     # Development with auto-reload
```

### Key Features

**Express Setup:**
```typescript
import express from 'express'
const app = express()
app.use(express.json())
app.use(cors())
```

**Service Implementation:**
```typescript
export class TodosService implements ITodosService {
    async listTodos(): Promise<Todo[]> { }
    async createTodo(input: CreateTodoInput): Promise<Todo> { }
    async updateTodo(id: string, input: UpdateTodoInput): Promise<Todo> { }
    async deleteTodo(id: string): Promise<void> { }
}
```

## Frontend Setup (Flutter)

### Prerequisites
- Flutter SDK
- Xcode (macOS/iOS) or Android Studio
- Physical device or emulator

### Installation

```bash
cd frontend
flutter pub get     # Download packages
flutter run        # Run on default device

# Or specify device
flutter run -d iphone     # iOS
flutter run -d android    # Android
flutter run -d chrome     # Web
```

### Key Flutter Features

**Stateful Widget:**
```dart
class TodoScreen extends StatefulWidget {
    @override
    State<TodoScreen> createState() => _TodoScreenState();
}

class _TodoScreenState extends State<TodoScreen> {
    List<Todo> todos = [];
    
    @override
    void initState() {
        super.initState();
        _loadTodos();
    }
    
    Future<void> _loadTodos() async {
        final fetched = await client.ListTodos();
        setState(() { todos = fetched; });
    }
}
```

**Async/Await:**
```dart
Future<void> addTodo(String title) async {
    try {
        final todo = await client.CreateTodo(
            CreateTodoInput(title: title, userId: '1')
        );
        setState(() { todos.add(todo); });
    } catch (e) {
        _showError('Failed to add todo: $e');
    }
}
```

**Material Design Widgets:**
```dart
ListView.builder(
    itemCount: todos.length,
    itemBuilder: (context, index) {
        return ListTile(
            title: Text(todos[index].title),
            trailing: IconButton(
                icon: Icon(Icons.delete),
                onPressed: () => _deleteTodo(todos[index].id),
            ),
        );
    },
)
```

## Architecture

### Flutter App Structure
```
lib/
├── main.dart                 # App entry
├── screens/
│   ├── todo_screen.dart
│   └── user_screen.dart
├── models/
│   ├── todo.dart
│   └── user.dart
├── generated/
│   └── client/api_client.dart
└── services/
    └── api_service.dart
```

### Backend → Frontend Connection
```
Backend (Express)
    ↓ HTTP
Generated API Client (Dart)
    ↓ Type-safe calls
Flutter Widgets
    ↓ User Interactions
Models & State Management
```

## Running Together

**Terminal 1 - Backend:**
```bash
cd backend
npm run dev
# localhost:3000
```

**Terminal 2 - Frontend:**
```bash
cd frontend
flutter run     # Connects to localhost:3000
```

The app will automatically sync with the backend!

## Type Safety in Dart

```dart
// Generated types from Veld
import 'generated/types/todo.dart';
import 'generated/types/user.dart';
import 'generated/client/api_client.dart';

// Full type checking
final Todo todo = await client.CreateTodo(
    CreateTodoInput(title: 'Buy milk', userId: '1')
);

// Compile-time safety
await client.CreateTodo(
    CreateTodoInput(
        title: 'Buy milk',
        // userId: REQUIRED - Error if missing!
    )
);
```

## State Management

**Simple State Approach:**
```dart
class _TodoScreenState extends State<TodoScreen> {
    List<Todo> todos = [];
    bool isLoading = true;
    String? error;
    
    @override
    void initState() {
        super.initState();
        _loadTodos();
    }
    
    Future<void> _loadTodos() async {
        try {
            setState(() => isLoading = true);
            todos = await client.ListTodos();
            error = null;
        } catch (e) {
            error = e.toString();
        } finally {
            setState(() => isLoading = false);
        }
    }
}
```

**Provider Pattern (optional):**
```dart
// pubspec.yaml
dependencies:
  provider: ^6.0.0

// Usage
class TodoProvider extends ChangeNotifier {
    List<Todo> _todos = [];
    
    Future<void> loadTodos() async {
        _todos = await client.ListTodos();
        notifyListeners();
    }
}
```

## Cross-Platform

**Platform-Specific Code:**
```dart
import 'dart:io' show Platform;

if (Platform.isIOS) {
    // iOS-specific code
} else if (Platform.isAndroid) {
    // Android-specific code
}
```

**Web/Desktop too:**
```bash
flutter run -d chrome   # Web
flutter run -d macos    # macOS
flutter run -d windows  # Windows
flutter run -d linux    # Linux
```

## Production

### Backend
```bash
npm run build
npm start
# Deploy to Heroku, Railway, etc.
```

### Frontend
```bash
flutter build ios       # Build for App Store
flutter build apk       # Build for Play Store
flutter build appbundle # Android App Bundle
flutter build web       # Web version
```

## Troubleshooting

### Flutter Issues
```bash
# Clean cache
flutter clean

# Get latest packages
flutter pub get

# Check environment
flutter doctor

# Upgrade Flutter
flutter upgrade
```

### Device Connection
```bash
# List connected devices
flutter devices

# Run on specific device
flutter run -d <device-id>

# Kill hanging processes
pkill dart
```

### Backend Connection
- Ensure backend running on localhost:3000
- Check firewall/network settings
- On emulator, use `10.0.2.2` for localhost
- On physical device, use actual IP address

## Resources

- [Flutter Documentation](https://flutter.dev/docs)
- [Dart Language Guide](https://dart.dev/guides)
- [Material Design](https://material.io/)
- [Cupertino Widgets (iOS)](https://flutter.dev/docs/development/ui/widgets/cupertino)
- [Veld Documentation](https://veld.dev)

---

Perfect for building **beautiful cross-platform apps**! 📱✨

