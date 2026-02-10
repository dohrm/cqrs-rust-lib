use super::commands::{CreateCommands, UpdateCommands};
use super::errors::ErrorCode;
use super::events::Events;
use cqrs_rust_lib::{
    Aggregate, CommandHandler, CqrsContext, CqrsError, CqrsErrorCode, EventEnvelope, View,
};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const AGGREGATE_TYPE: &str = "todolist";

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct TodoList {
    pub id: String,
    pub name: String,
    pub todos: Vec<Todo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub assignee: Option<String>,
    pub resolved: bool,
}

#[async_trait::async_trait]
impl Aggregate for TodoList {
    const TYPE: &'static str = AGGREGATE_TYPE;

    type Event = Events;
    type Error = CqrsError;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            Events::TodoListCreated { name } => {
                self.name = name;
            }
            Events::TodoAdded { todo_id, title } => self.todos.push(Todo {
                id: todo_id,
                title,
                assignee: None,
                resolved: false,
            }),
            Events::TodoRemoved { todo_id } => {
                self.todos.retain(|t| t.id != todo_id);
            }
            Events::TodoAssignedTo { todo_id, assignee } => {
                if let Some(t) = self.todos.iter_mut().find(|t| t.id == todo_id) {
                    t.assignee = Some(assignee);
                }
            }
            Events::TodoResolved { todo_id } => {
                if let Some(t) = self.todos.iter_mut().find(|t| t.id == todo_id) {
                    t.resolved = true;
                }
            }
        }
        Ok(())
    }

    fn error(status: StatusCode, details: &str) -> Self::Error {
        CqrsError::from_status(status, details)
    }
}

#[async_trait::async_trait]
impl CommandHandler for TodoList {
    type CreateCommand = CreateCommands;
    type UpdateCommand = UpdateCommands;
    type Services = ();

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommands::Create { name } => {
                if name.trim().is_empty() {
                    return Err(ErrorCode::ListNameRequired.error("List name cannot be empty"));
                }
                Ok(vec![Events::TodoListCreated { name }])
            }
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommands::AddTodo { title } => {
                if title.trim().is_empty() {
                    return Err(ErrorCode::EmptyTitle.error("Todo title cannot be empty"));
                }
                Ok(vec![Events::TodoAdded {
                    todo_id: context.next_uuid(),
                    title,
                }])
            }
            UpdateCommands::RemoveTodo { todo_id } => {
                if !self.todos.iter().any(|t| t.id == todo_id) {
                    return Err(ErrorCode::TodoNotFound
                        .error(format!("Todo '{}' not found", todo_id)));
                }
                Ok(vec![Events::TodoRemoved { todo_id }])
            }
            UpdateCommands::AssignTodo { todo_id, assignee } => {
                if !self.todos.iter().any(|t| t.id == todo_id) {
                    return Err(ErrorCode::TodoNotFound
                        .error(format!("Todo '{}' not found", todo_id)));
                }
                Ok(vec![Events::TodoAssignedTo { todo_id, assignee }])
            }
            UpdateCommands::ResolveTodo { todo_id } => {
                let todo = self.todos.iter().find(|t| t.id == todo_id);
                match todo {
                    None => Err(ErrorCode::TodoNotFound
                        .error(format!("Todo '{}' not found", todo_id))),
                    Some(t) if t.resolved => Err(ErrorCode::TodoAlreadyResolved
                        .error(format!("Todo '{}' is already resolved", todo_id))),
                    _ => Ok(vec![Events::TodoResolved { todo_id }]),
                }
            }
        }
    }
}

impl View<TodoList> for TodoList {
    const TYPE: &'static str = AGGREGATE_TYPE;
    const IS_CHILD_OF_AGGREGATE: bool = false;

    fn view_id(event: &EventEnvelope<TodoList>) -> String {
        event.aggregate_id.to_string()
    }

    fn update(&self, _event: &EventEnvelope<TodoList>) -> Option<Self> {
        None
    }
}
