using VeldGenerated.Models;
using VeldGenerated.Services;

namespace VeldExample.Services;

// In-memory implementation of the Veld-generated IUsersService interface.
public class UsersService : IUsersService
{
    private readonly List<User> _users =
    [
        new User("1", "Alice", "alice@example.com"),
        new User("2", "Bob",   "bob@example.com"),
    ];

    public Task<List<User>> ListUsers() =>
        Task.FromResult(_users.ToList());

    public Task<User> GetUser(string Id)
    {
        var user = _users.FirstOrDefault(u => u.Id == Id)
            ?? throw new KeyNotFoundException($"User {Id} not found");
        return Task.FromResult(user);
    }

    public Task<User> CreateUser(CreateUserInput input)
    {
        var user = new User(Guid.NewGuid().ToString(), input.Name, input.Email);
        _users.Add(user);
        return Task.FromResult(user);
    }

    public Task DeleteUser(string Id)
    {
        _users.RemoveAll(u => u.Id == Id);
        return Task.CompletedTask;
    }
}
