# PHP + TypeScript Example Setup Guide

## Quick Start

```bash
# Backend (PHP/Laravel)
cd php-typescript/backend && composer install && php artisan serve
# Runs on http://localhost:8000

# Frontend (TypeScript)
cd php-typescript/frontend && npm install && npm start
# Runs on http://localhost:3000
```

## Backend Setup (PHP / Laravel)

### Prerequisites
- PHP 8.2+
- Composer

### Installation

```bash
cd backend

# Install dependencies
composer install

# Generate app key
php artisan key:generate

# Run migrations (if using database)
php artisan migrate

# Start development server
php artisan serve
# Runs on http://localhost:8000
```

### Project Structure

```
backend/
├── app/
│   ├── Services/
│   │   ├── TodosService.php
│   │   └── UsersService.php
│   ├── Models/
│   │   ├── Todo.php
│   │   └── User.php
│   ├── Http/
│   │   └── Controllers/
│       ├── TodosController.php
│       └── UsersController.php
├── routes/
│   └── api.php
├── database/
│   └── migrations/
├── composer.json
└── .env.example
```

### Key Laravel Features

**Service Implementation:**
```php
class TodosService implements ITodosService
{
    private static array $store = [];
    
    public function listTodos(): array
    {
        return self::$store;
    }
    
    public function createTodo(CreateTodoInput $input): Todo
    {
        $todo = new Todo(
            id: uniqid('t'),
            title: $input->title,
            completed: false,
            userId: $input->userId
        );
        self::$store[] = $todo;
        return $todo;
    }
}
```

**Route Registration:**
```php
// routes/api.php
Route::middleware('api')->group(function () {
    Route::get('/todos', [TodosController::class, 'list']);
    Route::post('/todos', [TodosController::class, 'create']);
    Route::put('/todos/{id}', [TodosController::class, 'update']);
    Route::delete('/todos/{id}', [TodosController::class, 'delete']);
});
```

**Type Hints:**
```php
function processUser(User $user): array
{
    return $user->toArray();
}

function createTodo(CreateTodoInput $input): Todo
{
    // Full type safety
}
```

## PHP Best Practices

### Named Arguments
```php
$todo = new Todo(
    id: '123',
    title: 'Buy milk',
    completed: false,
    userId: '1'
);
```

### Match Expression
```php
return match($status) {
    'pending' => 'Waiting...',
    'completed' => 'Done!',
    'failed' => 'Error',
    default => 'Unknown'
};
```

### Attributes (Decorators)
```php
#[Route('/todos', 'POST')]
#[Middleware('auth')]
public function create(CreateTodoInput $input): Todo { }
```

## Frontend Setup (TypeScript)

### Installation

```bash
cd frontend
npm install
npm start
```

**Same as node-typescript example** - use generated API client.

### Available Scripts

```bash
npm start              # Development server
npm run build         # Production build
npm run test          # Run tests
npm run type-check    # Check types
```

## Running Together

### Terminal 1 - Laravel Backend
```bash
cd backend
php artisan serve
# http://localhost:8000
```

### Terminal 2 - TypeScript Frontend
```bash
cd frontend
npm start
# http://localhost:3000
```

Frontend automatically proxies to backend.

## Laravel + Veld Integration

### Environment Setup

**.env:**
```bash
APP_NAME=Veld
APP_ENV=local
APP_DEBUG=true
APP_URL=http://localhost:8000

DB_CONNECTION=sqlite
DB_DATABASE=database/database.sqlite

CORS_ALLOWED_ORIGINS=http://localhost:3000
```

### CORS Configuration

**config/cors.php:**
```php
'allowed_origins' => ['http://localhost:3000'],
'allowed_methods' => ['*'],
'allowed_headers' => ['*'],
```

### Request Validation

```php
public function create(Request $request): Todo
{
    $validated = $request->validate([
        'title' => 'required|string|max:255',
        'userId' => 'required|string',
    ]);
    
    return service.createTodo(
        new CreateTodoInput(
            title: $validated['title'],
            userId: $validated['userId']
        )
    );
}
```

## Database Integration

### Migrations

**database/migrations/create_todos_table.php:**
```php
Schema::create('todos', function (Blueprint $table) {
    $table->id();
    $table->string('title');
    $table->boolean('completed')->default(false);
    $table->string('user_id');
    $table->timestamps();
});
```

**Create migration:**
```bash
php artisan make:migration create_todos_table
php artisan migrate
```

### Eloquent Models

```php
class Todo extends Model
{
    protected $fillable = ['title', 'completed', 'user_id'];
    
    public function user()
    {
        return $this->belongsTo(User::class, 'user_id');
    }
}
```

**Query Builder:**
```php
$todos = Todo::where('completed', false)
    ->orderBy('created_at')
    ->get();
```

## Type Safety in TypeScript Frontend

```typescript
import { api } from './generated/client/api'
import type { Todo, User } from './generated/types'

// Full type checking
const todos: Todo[] = await api.Todos.ListTodos()
const user: User = await api.Users.GetUser('123')

// Error on missing required fields
const todo = await api.Todos.CreateTodo({
    title: 'Buy milk',
    userId: '1'
    // All required fields typed
})
```

## Testing

### Backend - PHPUnit

```bash
php artisan test
```

**tests/Feature/TodosTest.php:**
```php
class TodosTest extends TestCase
{
    public function test_list_todos()
    {
        $response = $this->getJson('/api/todos');
        $response->assertStatus(200);
        $response->assertIsArray();
    }
}
```

### Frontend - Jest

```bash
npm test
```

**src/__tests__/api.test.ts:**
```typescript
import { api } from '../generated/client/api'

describe('API Integration', () => {
    it('should list todos', async () => {
        const todos = await api.Todos.ListTodos()
        expect(Array.isArray(todos)).toBe(true)
    })
})
```

## Production Deployment

### Backend (PHP)

```bash
# Optimize for production
composer install --no-dev --optimize-autoloader

# Build assets
php artisan optimize:clear

# Deploy to: 
# - Laravel Forge
# - Heroku
# - PythonAnywhere
# - AWS Elastic Beanstalk
# - Digital Ocean
# - Traditional shared hosting
```

### Frontend (TypeScript)

```bash
npm run build
# dist/ folder ready for deployment
```

Deploy to: Vercel, Netlify, Heroku, etc.

## Troubleshooting

### PHP Issues

```bash
# Check PHP version
php -v                    # Should be 8.2+

# Clear Laravel cache
php artisan cache:clear
php artisan config:clear
php artisan view:clear

# Check logs
tail -f storage/logs/laravel.log
```

### Composer Issues

```bash
# Clear cache
composer clear-cache

# Update dependencies
composer update

# Check for security vulnerabilities
composer audit
```

### Database Issues

```bash
# Fresh migrate
php artisan migrate:fresh

# Seed test data
php artisan db:seed
```

## Artisan Commands Cheat Sheet

```bash
php artisan serve              # Start dev server
php artisan make:model Todo    # Create model
php artisan make:controller    # Create controller
php artisan make:migration     # Create migration
php artisan migrate            # Run migrations
php artisan tinker            # Interactive shell
php artisan queue:work        # Start queue
php artisan cache:clear       # Clear cache
```

## Resources

- [Laravel Documentation](https://laravel.com/docs)
- [PHP 8.2 Guide](https://www.php.net/manual/en/migration82.php)
- [Eloquent ORM](https://laravel.com/docs/eloquent)
- [Laravel Testing](https://laravel.com/docs/testing)
- [Veld Documentation](https://veld.dev)

---

Perfect for **PHP developers** building **modern type-safe APIs**! 🚀

