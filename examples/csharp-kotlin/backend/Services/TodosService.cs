using VeldGenerated.Models;
using VeldGenerated.Services;

namespace VeldExample.Services;

// In-memory implementation of the Veld-generated ITodosService interface.
public class TodosService : ITodosService
{
    private readonly List<Todo> _todos =
    [
        new Todo("1", "Buy groceries", false, "1"),
        new Todo("2", "Write tests",   false, "2"),
    ];

    public Task<List<Todo>> ListTodos() =>
        Task.FromResult(_todos.ToList());

    public Task<Todo> GetTodo(string Id)
    {
        var todo = _todos.FirstOrDefault(t => t.Id == Id)
            ?? throw new KeyNotFoundException($"Todo {Id} not found");
        return Task.FromResult(todo);
    }

    public Task<Todo> CreateTodo(CreateTodoInput input)
    {
        var todo = new Todo(Guid.NewGuid().ToString(), input.Title, false, input.UserId);
        _todos.Add(todo);
        return Task.FromResult(todo);
    }

    public Task<Todo> UpdateTodo(string Id, UpdateTodoInput input)
    {
        var todo = _todos.FirstOrDefault(t => t.Id == Id)
            ?? throw new KeyNotFoundException($"Todo {Id} not found");
        var updated = todo with
        {
            Title     = input.Title     ?? todo.Title,
            Completed = input.Completed ?? todo.Completed,
        };
        _todos[_todos.IndexOf(todo)] = updated;
        return Task.FromResult(updated);
    }

    public Task DeleteTodo(string Id)
    {
        _todos.RemoveAll(t => t.Id == Id);
        return Task.CompletedTask;
    }
}
