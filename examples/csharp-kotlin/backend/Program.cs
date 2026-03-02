// ASP.NET Core Minimal API entry point.
// Generated controllers live in ../generated/Controllers/ and are picked up
// automatically by AddControllers() because VeldGenerated is a project reference.

using VeldExample.Services;
using VeldGenerated.Services;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddControllers();

// Register service implementations against the Veld-generated interfaces.
builder.Services.AddSingleton<IUsersService, UsersService>();
builder.Services.AddSingleton<ITodosService, TodosService>();

var app = builder.Build();

app.MapControllers();

app.Run("http://localhost:5000");
