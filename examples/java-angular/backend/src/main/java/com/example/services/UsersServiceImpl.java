package com.example.services;

import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

// Generated types — produced by `veld generate`
import com.example.generated.types.User;
import com.example.generated.types.CreateUserInput;
import com.example.generated.interfaces.IUsersService;

/**
 * In-memory implementation of the Veld-generated IUsersService interface.
 * Replace the ArrayList store with a real repository (e.g. Spring Data JPA)
 * without touching any generated files.
 */
@Service
public class UsersServiceImpl implements IUsersService {

    private final List<User> store = new ArrayList<>();

    @Override
    public List<User> listUsers() {
        return new ArrayList<>(store);
    }

    @Override
    public User getUser(String id) {
        return store.stream()
                .filter(u -> u.getId().equals(id))
                .findFirst()
                .orElseThrow(() -> new RuntimeException("User not found: " + id));
    }

    @Override
    public User createUser(CreateUserInput input) {
        User user = new User(UUID.randomUUID().toString(), input.getName(), input.getEmail());
        store.add(user);
        return user;
    }

    @Override
    public void deleteUser(String id) {
        store.removeIf(u -> u.getId().equals(id));
    }
}
