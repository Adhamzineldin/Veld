<?php

declare(strict_types=1);

namespace App\Services;

use App\Models\User;
use App\Models\CreateUserInput;

class UsersService implements IUsersService
{
    /** @var User[] */
    private static array $store = [];

    private static bool $seeded = false;

    public function __construct()
    {
        if (!self::$seeded) {
            self::$store = [
                new User(id: '1', name: 'Alice', email: 'alice@example.com'),
                new User(id: '2', name: 'Bob',   email: 'bob@example.com'),
            ];
            self::$seeded = true;
        }
    }

    public function listUsers(): array
    {
        return self::$store;
    }

    public function getUser(string $id): User
    {
        foreach (self::$store as $user) {
            if ($user->id === $id) {
                return $user;
            }
        }
        throw new \RuntimeException("User {$id} not found", 404);
    }

    public function createUser(CreateUserInput $input): User
    {
        $user = new User(id: uniqid('u', true), name: $input->name, email: $input->email);
        self::$store[] = $user;
        return $user;
    }

    public function deleteUser(string $id): void
    {
        foreach (self::$store as $i => $user) {
            if ($user->id === $id) {
                array_splice(self::$store, $i, 1);
                return;
            }
        }
        throw new \RuntimeException("User {$id} not found", 404);
    }
}
