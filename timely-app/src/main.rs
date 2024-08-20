use iced::widget::keyed::column;
use iced::widget::shader::wgpu::hal::Alignments;
use iced::widget::{
    button, checkbox, column, container, horizontal_space, keyed_column, radio, row, scrollable,
    slider, text, text_input, toggler, vertical_space, Column, Container, Row, Text,
};
use iced::{
    font, Application, Color, Command, Element, Font, Length, Pixels, Settings, Size, Theme,
};
use log::{info, Level};
use once_cell::sync::Lazy;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://sienkiewicza114.duckdns.org/timely";
static INPUT_ID: Lazy<text_input::Id> = Lazy::new(text_input::Id::unique);

use timely_lib::{add_to_hierarchy, build_hierarchy, Todo, TodoHierarchy};

// Fonts
const ICONS: Font = Font::with_name("Iced-Todos-Icons");

fn icon(unicode: char) -> Text<'static> {
    text(unicode.to_string()).font(ICONS).width(20)
}

fn edit_icon() -> Text<'static> {
    icon('\u{F303}')
}

fn delete_icon() -> Text<'static> {
    icon('\u{F1F8}')
}

pub fn todo_hierarchy_view<'a>(hierarchy: TodoHierarchy) -> Element<'a, Message> {
    // Create a row for the current todo item
    let mut row: Row<'a, Message> = row![
        text(&hierarchy.todo.name).size(24),
        if let Some(desc) = &hierarchy.todo.description {
            text(desc)
        } else {
            text("")
        },
        text(if hierarchy.todo.done {
            "Done"
        } else {
            "Pending"
        })
        .size(16),
        button(row![delete_icon(), "Delete"].spacing(10))
            .on_press(Message::DeleteTodo(hierarchy.todo.id))
            .padding(8),
    ]
    .spacing(2);

    // Add the children recursively
    for child in hierarchy.children {
        row = row.push(
            Container::new(todo_hierarchy_view(child))
                .padding(10)
                .width(Length::Fill),
        );
    }

    Container::new(row).padding(10).width(Length::Fill).into()
}

#[derive(Serialize)]
struct TodoToSend {
    name: String,
    description: String,
    parent_id: Option<i64>,
}

#[derive(Serialize)]
struct TodoToDelete {
    id: i64,
}

#[derive(Debug, Clone)]
enum Error {
    APIError,
    OtherError,
}
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Error {
        dbg!(error);
        Error::APIError
    }
}

#[derive(Debug)]
enum AppState {
    Loading,
    Loaded(String),
    AddingNewTodo(String, String, Option<i64>),
    SubmittingNewTodo(String, String, Option<i64>),
    Errored(String),
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Vec<Todo>, Error>),
    Load,
    LoadScreenAddNewTodo(String, String, Option<i64>),
    SubmitNewTodo(String, String, Option<i64>),
    SubmittedNewTodo(Result<Todo, Error>),
    GoneBackToMain,
    ToggleTodo(i64),
    DeleteTodo(i64),
    FontLoaded(Result<(), font::Error>),
    None,
}

async fn load(client: Client) -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = client
        .get(format!("{}/todos", API_URL))
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn submit_new_todo(todo_to_send: TodoToSend, client: Client) -> Result<Todo, Error> {
    let response: Todo = client
        .post(format!("{}/todos", API_URL))
        .json(&todo_to_send)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn delete_todo(id: i64, client: Client) -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = client
        .delete(format!("{}/todos", API_URL))
        .json(&TodoToDelete { id })
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn toggle_todo(id: i64, client: Client) -> Result<bool, Error> {
    let response: bool = client
        .post(format!("{}/todos/toggle", API_URL))
        .json(&TodoToDelete { id })
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

#[derive(Debug)]
struct App {
    state: AppState,
    todos: Option<Vec<TodoHierarchy>>,
    client: Client,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let client = Client::new();
        let command = Command::batch([
            font::load(include_bytes!("../fonts/icons.ttf").as_slice()).map(Message::FontLoaded),
            Command::perform(load(client.clone()), Message::Loaded),
        ]);

        let app = App {
            state: AppState::Loading,
            todos: None,
            client,
        };

        (app, command)
    }

    fn title(&self) -> String {
        let subtitle = match self.state {
            AppState::Loading => "Loading - ",
            AppState::Loaded(..) => "",
            AppState::Errored { .. } => "Error - ",
            AppState::AddingNewTodo(..) => "Adding new task - ",
            AppState::SubmittingNewTodo(..) => "Submitting task - ",
        };

        format!("{subtitle}Timely")
    }
    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::Loaded(todos_result) => match todos_result {
                Ok(todos) => {
                    let hierarchy = build_hierarchy(todos);
                    self.state = AppState::Loaded("".to_owned());
                    self.todos = Some(hierarchy);
                    Command::none()
                }
                Err(todos_error) => {
                    self.state = AppState::Errored(format!("{:?}", todos_error));
                    Command::none()
                }
            },
            Message::Load => Command::perform(load(self.client.clone()), Message::Loaded),
            Message::ToggleTodo(todo_id) => todo!(),
            Message::DeleteTodo(todo_id) => {
                Command::perform(delete_todo(todo_id, self.client.clone()), Message::Loaded)
            }
            Message::None => Command::none(),
            Message::LoadScreenAddNewTodo(title, description, parent_id) => {
                self.state = AppState::AddingNewTodo(title, description, parent_id);
                Command::none()
            }
            Message::GoneBackToMain => {
                self.state = AppState::Loaded("".to_owned());
                Command::none()
            }
            Message::SubmitNewTodo(name, description, parent_id) => Command::perform(
                submit_new_todo(
                    TodoToSend {
                        name,
                        description,
                        parent_id,
                    },
                    self.client.clone(),
                ),
                Message::SubmittedNewTodo,
            ),
            Message::SubmittedNewTodo(todo) => {
                if todo.is_ok() {
                    add_to_hierarchy(&mut self.todos.as_mut().unwrap(), todo.unwrap());
                    self.state = AppState::Loaded("".to_owned());
                }
                Command::none()
            }
            Message::FontLoaded(_) => Command::none(),
        }
    }

    fn view(&self) -> Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        let content: Element<_> =
            match &self.state {
                AppState::Loading => text("Loading...").into(),
                AppState::Loaded(input_value) => {
                    match self.todos.as_ref().unwrap().len() {
                        0 => text("No todos!").into(),
                        _ => column![
                            scrollable(
                                keyed_column(self.todos.as_ref().unwrap().iter().map(|todo| {
                                    (todo.todo.id, todo_hierarchy_view(todo.clone()))
                                }))
                                .spacing(10),
                            ),
                            row![
                                button("Add new").on_press(Message::LoadScreenAddNewTodo(
                                    "".into(),
                                    "".into(),
                                    None
                                )),
                                button("Refresh").on_press(Message::Load)
                            ]
                            .spacing(10)
                        ]
                        .into(),
                    }
                }
                AppState::Errored(error) => text(format!("Error: {}", error)).into(),
                AppState::AddingNewTodo(name, description, parent_id) => column![
                    button("Go back").on_press(Message::GoneBackToMain),
                    text_input("Task name", &name).on_input(|new_name| {
                        Message::LoadScreenAddNewTodo(new_name, description.clone(), *parent_id)
                    }),
                    text_input("Task description", &description).on_input(|new_description| {
                        Message::LoadScreenAddNewTodo(name.clone(), new_description, *parent_id)
                    }),
                    button("Submit").on_press(Message::SubmitNewTodo(
                        name.clone(),
                        description.clone(),
                        *parent_id
                    ))
                ]
                .into(),
                AppState::SubmittingNewTodo(name, _, _) => {
                    text(format!("Submitting todo: {}...", name)).into()
                }
            };
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .into()
    }
}

fn main() -> iced::Result {
    App::run(Settings {
        antialiasing: true,
        ..Default::default()
    })
}
