package com.example.services;

import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

// Generated types — produced by `veld generate`
import com.example.generated.types.Todo;
import com.example.generated.types.CreateTodoInput;
import com.example.generated.types.UpdateTodoInput;
import com.example.generated.interfaces.ITodosService;

/**
 * In-memory implementation of the Veld-generated ITodosService interface.
 * Replace the ArrayList store with a real repository (e.g. Spring Data JPA)
 * without touching any generated files.
 */
@Service
public class TodosServiceImpl implements ITodosService {

    private final List<Todo> store = new ArrayList<>();

    @Override
    public List<Todo> listTodos() {
        return new ArrayList<>(store);
    }

    @Override
    public Todo getTodo(String id) {
        return store.stream()
                .filter(t -> t.getId().equals(id))
                .findFirst()
                .orElseThrow(() -> new RuntimeException("Todo not found: " + id));
    }

    @Override
    public Todo createTodo(CreateTodoInput input) {
        Todo todo = new Todo(UUID.randomUUID().toString(), input.getTitle(), false, input.getUserId());
        store.add(todo);
        return todo;
    }

    @Override
    public Todo updateTodo(String id, UpdateTodoInput input) {
        Todo existing = getTodo(id);
        String title = input.getTitle() != null ? input.getTitle() : existing.getTitle();
        boolean completed = input.getCompleted() != null ? input.getCompleted() : existing.isCompleted();
        Todo updated = new Todo(id, title, completed, existing.getUserId());
        store.replaceAll(t -> t.getId().equals(id) ? updated : t);
        return updated;
    }

    @Override
    public void deleteTodo(String id) {
        store.removeIf(t -> t.getId().equals(id));
    }
}
