// Axum entry point — wires in-memory service implementations to generated routes.
// Run with: cargo run

use std::net::SocketAddr;
use std::sync::Arc;

mod services;

// Generated modules (run `veld generate` first).
mod models;
mod router;
mod users;
mod todos;

use services::{InMemoryUsersService, InMemoryTodosService};

#[tokio::main]
async fn main() {
    let users_svc = Arc::new(InMemoryUsersService::new()) as Arc<dyn generated::UsersService>;
    let todos_svc = Arc::new(InMemoryTodosService::new()) as Arc<dyn generated::TodosService>;

    let app = router::build_router()
        .with_state(users_svc)
        .with_state(todos_svc);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
