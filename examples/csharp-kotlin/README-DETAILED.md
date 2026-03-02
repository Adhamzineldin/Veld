# C# + Kotlin (Android) Example Setup Guide

## Quick Start

```bash
# Backend - C# / ASP.NET Core
cd csharp-kotlin/backend
dotnet restore
dotnet run
# Runs on http://localhost:5000

# Frontend - Kotlin / Android
cd csharp-kotlin/frontend
# Open in Android Studio and run on emulator or device
```

## Backend Setup (C# / ASP.NET Core)

### Prerequisites
- .NET 7+
- Visual Studio, VS Code, or JetBrains Rider

### Installation

```bash
cd backend
dotnet restore      # Download NuGet packages
dotnet run         # Run development server
dotnet watch run   # Run with file watching
```

### Key ASP.NET Core Features

**Program.cs Setup:**
```csharp
var builder = WebApplication.CreateBuilder(args);

// Add services
builder.Services.AddScoped<ITodosService, TodosService>();
builder.Services.AddScoped<IUsersService, UsersService>();
builder.Services.AddCors(options => { });

var app = builder.Build();

// Configure middleware
app.UseCors("AllowAll");
app.MapPost("/todos", HandleCreateTodo);

app.Run();
```

**Service Implementation:**
```csharp
public class TodosService : ITodosService
{
    private readonly List<Todo> _todos = new();
    
    public Task<List<Todo>> ListTodos() =>
        Task.FromResult(_todos.ToList());
    
    public async Task<Todo> CreateTodo(CreateTodoInput input)
    {
        var todo = new Todo { /* ... */ };
        _todos.Add(todo);
        return todo;
    }
}
```

**Record Types (C# 9+):**
```csharp
public record Todo(
    string Id,
    string Title,
    bool Completed,
    string UserId
);

public record CreateTodoInput(string Title, string UserId);
```

## Frontend Setup (Kotlin / Android)

### Prerequisites
- Android Studio (latest)
- Java 11+
- Android SDK 29+

### Installation

```bash
cd frontend
# Open in Android Studio

# Build and run
./gradlew build      # Build APK
./gradlew installDebug && adb shell am start -n com.example/.MainActivity
```

### Key Kotlin Features

**Coroutines:**
```kotlin
viewModelScope.launch {
    try {
        val todos = withContext(Dispatchers.IO) {
            api.Todos.listTodos()
        }
        _todos.value = todos
    } catch (e: Exception) {
        _error.value = e.message
    }
}
```

**Data Classes:**
```kotlin
data class Todo(
    val id: String,
    val title: String,
    val completed: Boolean,
    val userId: String
)

data class CreateTodoInput(
    val title: String,
    val userId: String
)
```

**ViewModel Pattern:**
```kotlin
class TodoViewModel : ViewModel() {
    private val _todos = MutableStateFlow<List<Todo>>(emptyList())
    val todos: StateFlow<List<Todo>> = _todos.asStateFlow()
    
    fun loadTodos() {
        viewModelScope.launch {
            _todos.value = api.Todos.listTodos()
        }
    }
}
```

## Architecture

### C# Backend
```
Program.cs
├── Services/
│   ├── TodosService.cs
│   └── UsersService.cs
├── Models/
│   ├── Todo.cs
│   ├── User.cs
│   └── Inputs/
└── Controllers/
    ├── TodosController.cs
    └── UsersController.cs
```

### Kotlin Frontend
```
MainActivity.kt
├── ui/
│   ├── TodoScreen.kt
│   └── UserScreen.kt
├── viewmodel/
│   ├── TodoViewModel.kt
│   └── UserViewModel.kt
└── api/
    └── VeldApi.kt  (generated)
```

## Running Together

```bash
# Backend
dotnet run     # localhost:5000

# Frontend
# Open in Android Studio and run on emulator or device
# Configure API URL to point to backend
```

## Kotlin Async Patterns

**Suspend Functions:**
```kotlin
suspend fun getTodos(): List<Todo> {
    return withContext(Dispatchers.IO) {
        api.Todos.listTodos()
    }
}

// Usage
viewModelScope.launch {
    val todos = getTodos()
}
```

**Flow (Reactive Streams):**
```kotlin
fun todosFlow(): Flow<List<Todo>> = flow {
    while (currentCoroutineContext().isActive) {
        emit(api.Todos.listTodos())
        delay(5000)  // Refresh every 5 seconds
    }
}
```

## Type Safety

**C# Type Checking:**
```csharp
// Compile-time type safety
var todo = new CreateTodoInput { Title = "Buy milk", UserId = "1" };
var result = await service.CreateTodo(todo);
// result is Todo, type-checked at compile time

// Null safety
string? nullableString = null;  // Allowed
string nonNullString = null;    // Compile error
```

**Kotlin Type Checking:**
```kotlin
// Nullable vs non-null types
var nonNull: String = "hello"
nonNull = null          // Compile error

var nullable: String? = "hello"
nullable = null         // OK

// Smart casts
val obj: Any = "string"
if (obj is String) {
    println(obj.length)  // obj automatically cast to String
}
```

## Production

### C# Backend
```bash
dotnet publish -c Release
# Runs on Azure, AWS, Heroku, Docker, etc.
```

### Kotlin Frontend
```bash
./gradlew assembleRelease  # Creates signed APK
# Upload to Google Play Store
```

## Troubleshooting

### C# / .NET
```bash
# Clear cache
rm -rf bin obj
dotnet clean
dotnet restore

# Check version
dotnet --version   # Should be 7+
```

### Kotlin / Android
```bash
# Clear Gradle cache
./gradlew clean
./gradlew build --refresh-dependencies

# Run specific test
./gradlew testDebugUnitTest
```

### Emulator Issues
```bash
# List available AVDs
emulator -list-avds

# Launch AVD
emulator -avd <avd_name>

# In Android Studio:
# Device Manager → Create new virtual device
```

## Resources

- [C# Documentation](https://docs.microsoft.com/dotnet/csharp/)
- [ASP.NET Core Docs](https://docs.microsoft.com/aspnet/core/)
- [Kotlin Documentation](https://kotlinlang.org/docs/)
- [Android Developer Guide](https://developer.android.com/guide)
- [Jetpack Compose](https://developer.android.com/jetpack/compose)
- [Veld Documentation](https://veld.dev)

---

Perfect for **.NET teams** building **Android** apps! 📱

