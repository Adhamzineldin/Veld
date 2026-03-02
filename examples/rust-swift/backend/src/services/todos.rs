// In-memory implementation of the generated TodosService trait.

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::models::{CreateTodoInput, Todo, UpdateTodoInput};
use crate::services::TodosService;

pub struct InMemoryTodosService {
    store: Arc<Mutex<Vec<Todo>>>,
}

impl InMemoryTodosService {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl TodosService for InMemoryTodosService {
    async fn list_todos(&self) -> Result<Vec<Todo>, Box<dyn std::error::Error>> {
        let store = self.store.lock().unwrap();
        Ok(store.clone())
    }

    async fn get_todo(&self, id: String) -> Result<Todo, Box<dyn std::error::Error>> {
        let store = self.store.lock().unwrap();
        store
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| format!("todo {} not found", id).into())
    }

    async fn create_todo(&self, input: CreateTodoInput) -> Result<Todo, Box<dyn std::error::Error>> {
        let todo = Todo {
            id: Uuid::new_v4().to_string(),
            title: input.title,
            completed: false,
            user_id: input.user_id,
        };
        self.store.lock().unwrap().push(todo.clone());
        Ok(todo)
    }

    async fn update_todo(
        &self,
        id: String,
        input: UpdateTodoInput,
    ) -> Result<Todo, Box<dyn std::error::Error>> {
        let mut store = self.store.lock().unwrap();
        let todo = store
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("todo {} not found", id))?;
        if let Some(title) = input.title {
            todo.title = title;
        }
        if let Some(completed) = input.completed {
            todo.completed = completed;
        }
        Ok(todo.clone())
    }

    async fn delete_todo(&self, id: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut store = self.store.lock().unwrap();
        let before = store.len();
        store.retain(|t| t.id != id);
        if store.len() == before {
            return Err(format!("todo {} not found", id).into());
        }
        Ok(())
    }
}
