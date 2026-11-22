package csharp

import (
	"os"
	"path/filepath"
)

// csharpSetupDocFile is separate from README.md so cmd/veld's writeGeneratedReadme
// (module index + regenerate hint) does not overwrite the C# integration guide.
const csharpSetupDocFile = "SETUP.md"

// recommendedProgramCs is example host entry code: controllers live in the VeldGenerated
// assembly, so AddApplicationPart is required for discovery.
const recommendedProgramCs = `using VeldGenerated.Controllers;
using VeldGenerated.Middleware;
using VeldGenerated.Services;
using Server.Data;
using Server.Services;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddSwaggerGen();

// MVC controllers from the referenced Veld generated assembly (not scanned by default).
builder.Services.AddControllers()
    .AddApplicationPart(typeof(AuthController).Assembly);

builder.Services.AddSingleton<InMemoryStore>();
builder.Services.AddHttpContextAccessor();
builder.Services.AddScoped<IAuthService, AuthService>();
builder.Services.AddScoped<IUsersService, UsersService>();

builder.Services.AddCors(options =>
{
    options.AddDefaultPolicy(policy =>
    {
        policy.WithOrigins(
                builder.Configuration["App:CorsOrigins"]?.Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                ?? ["http://localhost:5173", "http://localhost:3000"])
            .AllowAnyHeader()
            .AllowAnyMethod();
    });
});

var app = builder.Build();

if (app.Environment.IsDevelopment())
{
    app.UseSwagger();
    app.UseSwaggerUI();
}

app.UseMiddleware<ErrorHandlingMiddleware>();
app.UseHttpsRedirection();
app.UseCors();
app.MapControllers();

app.Run();
`

const readmeBeforeProgram = `# Veld-generated C# backend

**README.md** in this directory is the index from **veld generate** (modules, regenerate command). **This file (SETUP.md)** is the C# host integration guide and is not overwritten by that step.

This folder is **auto-generated** — do not edit generated **.cs** files by hand. Re-run **veld generate** after you change your **.veld** contracts.

## What is here

- **VeldGenerated.csproj** — library project referencing ASP.NET Core (**Microsoft.AspNetCore.App**).
- **Models/** — records and enums matching your contract.
- **Services/** — **I{Module}Service** interfaces you implement in your app.
- **Controllers/** — API controllers that call those interfaces.
- **Middleware/** — **ErrorHandlingMiddleware** and optional module middleware interfaces.

Your **host** project (the one you run with **dotnet run**) stays separate: it references this project and registers your service implementations.

---

## Walkthrough: new Web API + Veld

### 1. Create the ASP.NET Core host

From your repo root (or wherever you want the server):

    dotnet new webapi -n MyProject -o server

This creates **server/MyProject.csproj** and **Program.cs**.

### 2. Add Veld contracts and config

Put your **.veld** file(s) and **veld.config.json** in a folder (for example **veld/**). Point **backend** at **csharp** and **out** at the directory where this README lives (often **generated** or **../generated**).

If the host project is **not** next to **veld.config.json**, set **backendDirectory** (or **backendDir**) to the folder that contains the host **.csproj** so **veld setup** can find it:

    {
      "input": "app.veld",
      "backend": "csharp",
      "frontend": "none",
      "out": "../generated",
      "backendDirectory": "../server"
    }

Adjust paths to match your layout.

### 3. Generate code

From the directory that contains **veld.config.json**:

    veld generate

### 4. Link the generated project to your host

Still from the config directory (so Veld can read **veld.config.json**):

    veld setup

This adds a **ProjectReference** from your host **.csproj** to **VeldGenerated.csproj**, using a path relative to the host project. If setup cannot find a **.csproj**, run it with **--backend-dir** pointing at the folder that contains the host project:

    veld setup --backend-dir=../server

If your generated output path is not the default, pass **--out** to match **veld.config.json**.

### 5. Implement the generated service interfaces

In your host project, add classes that implement **I{Module}Service** from **VeldGenerated.Services** (one implementation per module). Use namespaces that match your app (the example below uses **Server.Data** and **Server.Services** — rename to your root namespace).

### 6. Recommended **Program.cs**

Controllers are compiled into the **VeldGenerated** assembly. **AddControllers()** alone only scans the host assembly, so register the generated assembly with **AddApplicationPart(typeof(SomeController).Assembly)** — use any controller type from your generated **Controllers/** folder (e.g. **AuthController** if you have an **Auth** module).

Wire **ErrorHandlingMiddleware** from **VeldGenerated.Middleware** if you use the generated error pipeline. Replace **AuthController**, **IAuthService**, **UsersService**, and **InMemoryStore** with types that match your contract and app.

`

const readmeAfterProgram = `### 7. Build and run

From the host project directory:

    dotnet build
    dotnet run

**dotnet build** must succeed: every **I{Module}Service** in **VeldGenerated.Services** should have a registered implementation, and **AddApplicationPart** must reference a real generated controller type.

---

## Quick reference

| Step | Command |
|------|---------|
| Emit code | veld generate |
| Patch host .csproj | veld setup |
| Custom backend folder | veld setup --backend-dir=PATH |
| Verify compile | dotnet build |

For more detail, see the main Veld documentation in the repository.
`

func csharpReadmeMarkdown() string {
	return readmeBeforeProgram + "```csharp\n" + recommendedProgramCs + "\n```\n\n" + readmeAfterProgram
}

func (e *CSharpEmitter) emitReadme(outDir string) error {
	path := filepath.Join(outDir, csharpSetupDocFile)
	return os.WriteFile(path, []byte(csharpReadmeMarkdown()), 0644)
}
