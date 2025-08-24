use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum CreateCommands {
    Create { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum UpdateCommands {
    AddTodo { title: String },
    RemoveTodo { todo_id: String },
    AssignTodo { todo_id: String, assignee: String },
    ResolveTodo { todo_id: String },
}
