# Java + Angular Example Setup Guide

## Quick Start

```bash
# Terminal 1 - Backend (requires Maven & Java 17+)
cd java-angular/backend
mvn spring-boot:run
# Runs on http://localhost:8080

# Terminal 2 - Frontend
cd java-angular/frontend
npm install
npm start
# Runs on http://localhost:4200
```

## Backend Setup (Java/Spring Boot)

### Prerequisites
- Java 17+
- Maven 3.9+

### Installation

```bash
cd backend
mvn clean install      # Download dependencies
mvn spring-boot:run    # Run server
```

### Key Spring Boot Features

**Service Injection:**
```java
@Service
public class TodosServiceImpl implements ITodosService {
    @Autowired
    private TodosRepository todosRepository;
}
```

**REST Controllers:**
```java
@RestController
@RequestMapping("/todos")
public class TodosController {
    @PostMapping
    public ResponseEntity<Todo> create(@RequestBody CreateTodoInput input) {
        return todosService.createTodo(input);
    }
}
```

**Type Safety:**
```java
@Entity
public class Todo {
    @Id
    private String id;
    private String title;
    private boolean completed;
    private String userId;
}
```

## Frontend Setup (Angular)

### Prerequisites
- Node.js 18+
- Angular CLI

### Installation

```bash
cd frontend
npm install
npm start               # Development server
ng build              # Production build
```

### Key Angular Features

**Components:**
```typescript
@Component({
    selector: 'app-todo',
    template: `
        <div *ngFor="let todo of todos">
            {{ todo.title }}
        </div>
    `
})
export class TodoComponent implements OnInit {
    todos: Todo[] = []
    
    ngOnInit() {
        this.loadTodos()
    }
}
```

**Dependency Injection:**
```typescript
@Component(...)
export class TodoComponent {
    constructor(private api: ApiService) {}
}
```

**RxJS Observables:**
```typescript
todos$: Observable<Todo[]>

ngOnInit() {
    this.todos$ = this.api.Todos.ListTodos()
}

// In template:
<div *ngFor="let todo of todos$ | async">
    {{ todo.title }}
</div>
```

**Type Checking:**
```typescript
// Full type support from generated interfaces
interface ITodosService {
    ListTodos(): Promise<Todo[]>
    CreateTodo(input: CreateTodoInput): Promise<Todo>
}
```

## Architecture

### Layered Architecture
```
Spring Boot:
- Controllers → Services → Repositories → Database

Angular:
- Components → Services → HTTP Client → Backend
```

### Spring Boot Project Structure
```
src/main/java/com/example/
├── TodosServiceImpl.java
├── UsersServiceImpl.java
├── TodosController.java
└── UsersController.java
```

### Angular Project Structure
```
src/app/
├── components/
│   ├── todo/
│   │   └── todo.component.ts
│   └── user/
│       └── user.component.ts
├── services/
│   ├── api.service.ts
│   └── state.service.ts
└── models/
    ├── todo.model.ts
    └── user.model.ts
```

## Running Together

```bash
# Backend
mvn spring-boot:run    # Port 8080

# Frontend (new terminal)
npm start             # Port 4200, auto-proxies to :8080
```

Visit `http://localhost:4200`

## Key Files

- `backend/src/main/java/.../TodosServiceImpl.java`
- `backend/src/main/java/.../TodosController.java`
- `frontend/src/app/components/todo.component.ts`
- `frontend/src/app/services/api.service.ts`

## Data Binding in Angular

**Two-way Binding:**
```html
<input [(ngModel)]="title" />
<p>Title: {{ title }}</p>
```

**Event Binding:**
```html
<button (click)="deleteTodo(id)">Delete</button>
<input (change)="onTitleChange($event)" />
```

**Property Binding:**
```html
<todo-item [todo]="selectedTodo" [disabled]="isLoading"></todo-item>
```

**Structural Directives:**
```html
<div *ngIf="loading">Loading...</div>
<div *ngFor="let todo of todos; let i = index">
    {{ i + 1 }}. {{ todo.title }}
</div>
<div [ngSwitch]="status">
    <div *ngSwitchCase="'pending'">Pending...</div>
    <div *ngSwitchDefault>Done!</div>
</div>
```

## Type Safety

```typescript
// Generated types
import { Todo, CreateTodoInput, ITodosService } from '../generated'

// Type-checked API calls
const todo: Todo = await api.Todos.CreateTodo(input)
const todos: Todo[] = await api.Todos.ListTodos()

// Compile-time errors
await api.Todos.CreateTodo({
    // title: REQUIRED - Error if missing!
    userId: '123'
})
```

## Services & State Management

**Shared Service:**
```typescript
@Injectable({ providedIn: 'root' })
export class ApiService {
    private todos$ = new BehaviorSubject<Todo[]>([])
    
    get todos() {
        return this.todos$.asObservable()
    }
    
    async loadTodos() {
        const todos = await api.Todos.ListTodos()
        this.todos$.next(todos)
    }
}
```

**Component Usage:**
```typescript
export class TodoComponent implements OnInit {
    todos$ = this.api.todos
    
    constructor(private api: ApiService) {}
    
    ngOnInit() {
        this.api.loadTodos()
    }
}
```

## Production

### Backend
```bash
mvn clean package    # Creates uber-jar
java -jar target/app.jar
```

Deploy to: AWS, Google Cloud, Heroku, Azure, etc.

### Frontend
```bash
ng build --configuration production  # dist/ folder
```

Deploy to: Netlify, Vercel, S3 + CloudFront, etc.

## Troubleshooting

### Java/Maven
```bash
# Clear Maven cache
rm -rf ~/.m2/repository
mvn clean install

# Check Java version
java -version          # Should be 17+
```

### Angular
```bash
# Clear Angular cache
rm -rf node_modules .angular
npm install

# Update CLI
npm install -g @angular/cli@latest
```

### Port Conflicts
```bash
# Java: Edit application.properties
server.port=8081

# Angular: Change port
ng serve --port 4201
```

## Resources

- [Spring Boot Documentation](https://spring.io/projects/spring-boot)
- [Angular Documentation](https://angular.io/docs)
- [RxJS Documentation](https://rxjs.dev/)
- [Veld Documentation](https://veld.dev)

---

Perfect for **enterprise Java teams** building modern frontends! 🏢

