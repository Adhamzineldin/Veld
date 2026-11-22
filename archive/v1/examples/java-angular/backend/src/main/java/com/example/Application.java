package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Entry point for the Veld java-angular example backend.
 *
 * Spring Boot auto-discovers the @Service implementations in com.example.services
 * and the generated route handlers (controllers) in the generated/ package.
 * No manual wiring is required — implement the interface, annotate with @Service,
 * and the routes are live.
 */
@SpringBootApplication
public class Application {

    public static void main(String[] args) {
        SpringApplication.run(Application.class, args);
    }
}
