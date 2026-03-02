<?php

// This file bootstraps the Veld-generated route registrations into Laravel.
// The generated file registers all Route:: entries for Users and Todos.
// Controllers are auto-resolved by Laravel's service container — wire your
// service implementations in app/Providers/AppServiceProvider.php:
//
//   $this->app->bind(IUsersService::class, UsersService::class);
//   $this->app->bind(ITodosService::class, TodosService::class);

require __DIR__ . '/../../generated/routes/api.php';
