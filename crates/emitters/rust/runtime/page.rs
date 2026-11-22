//! `Page<T>`: one page of a list.

use serde::{Deserialize, Serialize};

/// One page of items. On the wire `total` is a decimal string, like every `long`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i32,
    pub size: i32,
}
