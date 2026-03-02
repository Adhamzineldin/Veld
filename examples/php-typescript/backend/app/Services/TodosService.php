<?php

declare(strict_types=1);

namespace App\Services;

use App\Models\Todo;
use App\Models\CreateTodoInput;
use App\Models\UpdateTodoInput;

class TodosService implements ITodosService
{
    /** @var Todo[] */
    private static array $store = [];

    private static bool $seeded = false;

    public function __construct()
    {
        if (!self::$seeded) {
            self::$store = [
                new Todo(id: '1', title: 'Buy groceries', completed: false, userId: '1'),
                new Todo(id: '2', title: 'Read Veld docs', completed: true,  userId: '2'),
            ];
            self::$seeded = true;
        }
    }

    public function listTodos(): array
    {
        return self::$store;
    }

    public function getTodo(string $id): Todo
    {
        foreach (self::$store as $todo) {
            if ($todo->id === $id) {
                return $todo;
            }
        }
        throw new \RuntimeException("Todo {$id} not found", 404);
    }

    public function createTodo(CreateTodoInput $input): Todo
    {
        $todo = new Todo(id: uniqid('t', true), title: $input->title, completed: false, userId: $input->userId);
        self::$store[] = $todo;
        return $todo;
    }

    public function updateTodo(string $id, UpdateTodoInput $input): Todo
    {
        foreach (self::$store as $i => $todo) {
            if ($todo->id === $id) {
                self::$store[$i] = new Todo(
                    id:        $todo->id,
                    title:     $input->title     ?? $todo->title,
                    completed: $input->completed ?? $todo->completed,
                    userId:    $todo->userId,
                );
                return self::$store[$i];
            }
        }
        throw new \RuntimeException("Todo {$id} not found", 404);
    }

    public function deleteTodo(string $id): void
    {
        foreach (self::$store as $i => $todo) {
            if ($todo->id === $id) {
                array_splice(self::$store, $i, 1);
                return;
            }
        }
        throw new \RuntimeException("Todo {$id} not found", 404);
    }
}
