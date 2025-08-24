use cqrs_rust_lib::Event;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum Events {
    TodoListCreated { name: String },
    TodoAdded { todo_id: String, title: String },
    TodoRemoved { todo_id: String },
    TodoAssignedTo { todo_id: String, assignee: String },
    TodoResolved { todo_id: String },
}

impl Event for Events {
    fn event_type(&self) -> String {
        match self {
            Events::TodoListCreated { .. } => "todo_list_created".into(),
            Events::TodoAdded { .. } => "todo_added".into(),
            Events::TodoRemoved { .. } => "todo_removed".into(),
            Events::TodoAssignedTo { .. } => "todo_assigned_to".into(),
            Events::TodoResolved { .. } => "todo_resolved".into(),
        }
    }
}
