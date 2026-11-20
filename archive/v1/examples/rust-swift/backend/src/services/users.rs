// In-memory implementation of the generated UsersService trait.

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::models::{CreateUserInput, UpdateUserInput, User};
use crate::services::UsersService;

pub struct InMemoryUsersService {
    store: Arc<Mutex<Vec<User>>>,
}

impl InMemoryUsersService {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl UsersService for InMemoryUsersService {
    async fn list_users(&self) -> Result<Vec<User>, Box<dyn std::error::Error>> {
        let store = self.store.lock().unwrap();
        Ok(store.clone())
    }

    async fn get_user(&self, id: String) -> Result<User, Box<dyn std::error::Error>> {
        let store = self.store.lock().unwrap();
        store
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or_else(|| format!("user {} not found", id).into())
    }

    async fn create_user(&self, input: CreateUserInput) -> Result<User, Box<dyn std::error::Error>> {
        let user = User {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            email: input.email,
        };
        self.store.lock().unwrap().push(user.clone());
        Ok(user)
    }

    async fn delete_user(&self, id: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut store = self.store.lock().unwrap();
        let before = store.len();
        store.retain(|u| u.id != id);
        if store.len() == before {
            return Err(format!("user {} not found", id).into());
        }
        Ok(())
    }
}
