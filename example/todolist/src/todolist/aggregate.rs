use super::commands::{CreateCommands, UpdateCommands};
use super::events::Events;
use cqrs_rust_lib::{Aggregate, CqrsContext, EventEnvelope, View};
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

    type CreateCommand = CreateCommands;
    type UpdateCommand = UpdateCommands;
    type Event = Events;
    type Services = ();
    type Error = std::io::Error;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }
    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommands::Create { name } => Ok(vec![Events::TodoListCreated { name }]),
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommands::AddTodo { title } => Ok(vec![Events::TodoAdded {
                todo_id: context.next_uuid(),
                title,
            }]),
            UpdateCommands::RemoveTodo { todo_id } => Ok(vec![Events::TodoRemoved { todo_id }]),
            UpdateCommands::AssignTodo { todo_id, assignee } => {
                Ok(vec![Events::TodoAssignedTo { todo_id, assignee }])
            }
            UpdateCommands::ResolveTodo { todo_id } => Ok(vec![Events::TodoResolved { todo_id }]),
        }
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

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::other(details.to_string())
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
