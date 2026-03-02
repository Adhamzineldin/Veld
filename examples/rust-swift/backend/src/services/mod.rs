// Service implementations — business logic lives here, not in generated code.

pub mod users;
pub mod todos;

pub use users::InMemoryUsersService;
pub use todos::InMemoryTodosService;
