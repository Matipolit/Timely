use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub name: String,
    pub done: bool,
    pub description: Option<String>,
}
