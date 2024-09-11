use config::{Config, ConfigError, File};
use iced::theme::Palette;
use iced::widget::{
    button, checkbox, column, container, keyed_column, row, scrollable, text, text_input, Column,
    Container, Text,
};
use iced::{
    alignment, font, Alignment, Application, Command, Element, Font, Length, Pixels, Settings,
    Size, Theme,
};
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env::home_dir;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;

use timely_lib::Todo;

// Settings

fn palette_map() -> HashMap<&'static str, Palette> {
    let mut map = HashMap::new();

    map.insert("catppuccin_latte", Palette::CATPPUCCIN_LATTE);
    map.insert("catppuccin_frappe", Palette::CATPPUCCIN_FRAPPE);
    map.insert("catppuccin_macchiato", Palette::CATPPUCCIN_MACCHIATO);
    map.insert("catppuccin_mocha", Palette::CATPPUCCIN_MOCHA);
    map.insert("light", Palette::LIGHT);
    map.insert("dark", Palette::DARK);
    map.insert("dracula", Palette::DRACULA);
    map.insert("kanagawa_wave", Palette::KANAGAWA_WAVE);
    map.insert("kanagawa_dragon", Palette::KANAGAWA_DRAGON);
    map.insert("kanagawa_lotus", Palette::KANAGAWA_LOTUS);
    map.insert("solarized_light", Palette::SOLARIZED_LIGHT);
    map.insert("solarized_dark", Palette::SOLARIZED_DARK);

    map
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppSettings {
    server_url: String,
    palette: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            server_url: "http://localhost:3000".into(),
            palette: "light".into(),
        }
    }
}

impl AppSettings {
    fn load() -> Result<Self, ConfigError> {
        let config_path = format!(
            "{}/.config/timely/Settings.toml",
            home_dir().unwrap().to_str().unwrap()
        );
        let config = Config::builder()
            .add_source(File::with_name(&config_path))
            .build()?;

        config.try_deserialize()
    }

    fn save(&self) -> std::io::Result<()> {
        let config_path = PathBuf::from(format!(
            "{}/.config/timely/Settings.toml",
            home_dir().unwrap().to_str().unwrap()
        ));
        let toml_string = toml::to_string(self).unwrap();
        fs::write(config_path, toml_string)
    }
}

// Fonts
const ICONS: Font = Font::with_name("Iced-Todos-Icons");

fn icon(unicode: char) -> Text<'static> {
    text(unicode.to_string())
        .font(ICONS)
        .size(16)
        .horizontal_alignment(alignment::Horizontal::Center)
        .vertical_alignment(alignment::Vertical::Center)
}

fn edit_icon() -> Text<'static> {
    icon('\u{F303}')
}

fn add_icon() -> Text<'static> {
    text("+")
        .size(16)
        .horizontal_alignment(alignment::Horizontal::Center)
        .vertical_alignment(alignment::Vertical::Center)
}

fn delete_icon() -> Text<'static> {
    icon('\u{F1F8}')
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
    Settings,
    Errored(String),
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Vec<Todo>, Error>),
    Load,
    LoadScreenAddNewTodo(String, String, Option<i64>),
    LoadScreenSettings,
    SubmitNewTodo(String, String, Option<i64>),
    SubmittedNewTodo(Result<Todo, Error>),
    GoBackToMain,
    TodoToggled(Result<(i64, bool), Error>),
    FontLoaded(Result<(), font::Error>),
    TodoMessage(i64, TodoMessage),
    ChangeUrl(String),
    SaveSettings,
    None,
}

async fn load(client: Client, url: String) -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = client
        .get(format!("{}/todos", url))
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn submit_new_todo(
    todo_to_send: TodoToSend,
    client: Client,
    url: String,
) -> Result<Todo, Error> {
    let response: Todo = client
        .post(format!("{}/todos", url))
        .json(&todo_to_send)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn delete_todo(id: i64, client: Client, url: String) -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = client
        .delete(format!("{}/todos", url))
        .json(&id)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn toggle_todo(id: i64, client: Client, url: String) -> Result<(i64, bool), Error> {
    let response: bool = client
        .post(format!("{}/todos/toggle", url))
        .json(&id)
        .send()
        .await?
        .json()
        .await?;
    Ok((id, response))
}

#[derive(Debug)]
struct App {
    state: AppState,
    todos: Vec<TodoHierarchy>,
    client: Client,
    palette: Palette,
    settings: AppSettings,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = AppSettings;

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let server_url = flags.server_url.clone();
        let client = Client::new();
        let command = Command::batch([
            font::load(include_bytes!("../fonts/icons.ttf").as_slice()).map(Message::FontLoaded),
            Command::perform(load(client.clone(), server_url.clone()), Message::Loaded),
        ]);

        let app = App {
            state: AppState::Loading,
            todos: Vec::new(),
            client,
            palette: *palette_map()
                .get(flags.palette.as_str())
                .unwrap_or(&Palette::LIGHT),
            settings: flags,
        };

        (app, command)
    }

    fn title(&self) -> String {
        let subtitle = match self.state {
            AppState::Loading => "Loading - ",
            AppState::Loaded(..) => "",
            AppState::Errored { .. } => "Error - ",
            AppState::AddingNewTodo(..) => "Adding new task - ",
            AppState::Settings => "Settings - ",
        };

        format!("{subtitle}Timely")
    }

    fn theme(&self) -> Self::Theme {
        Theme::custom("user_theme".into(), self.palette)
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::Loaded(todos_result) => match todos_result {
                Ok(todos) => {
                    let hierarchy = build_hierarchy(todos);
                    self.state = AppState::Loaded("".to_owned());
                    self.todos = hierarchy;
                    Command::none()
                }
                Err(todos_error) => {
                    self.state = AppState::Errored(format!("{:?}", todos_error));
                    Command::none()
                }
            },
            Message::Load => Command::perform(
                load(self.client.clone(), self.settings.server_url.clone()),
                Message::Loaded,
            ),
            Message::None => Command::none(),
            Message::LoadScreenAddNewTodo(title, description, parent_id) => {
                self.state = AppState::AddingNewTodo(title, description, parent_id);
                Command::none()
            }
            Message::GoBackToMain => {
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
                    self.settings.server_url.clone(),
                ),
                Message::SubmittedNewTodo,
            ),
            Message::SubmittedNewTodo(todo) => {
                if todo.is_ok() {
                    add_to_hierarchy(&mut self.todos, todo.unwrap());
                    self.state = AppState::Loaded("".to_owned());
                }
                Command::none()
            }
            Message::FontLoaded(_) => Command::none(),
            Message::TodoMessage(id, message) => {
                if let Some(_todo) = TodoHierarchy::get_hierarchy_by_id(&mut self.todos, id) {
                    match message {
                        TodoMessage::Done(id, _state) => Command::perform(
                            toggle_todo(id, self.client.clone(), self.settings.server_url.clone()),
                            Message::TodoToggled,
                        ),
                        TodoMessage::Delete(id) => Command::perform(
                            delete_todo(id, self.client.clone(), self.settings.server_url.clone()),
                            Message::Loaded,
                        ),
                        TodoMessage::AddChild(parent_id) => {
                            self.state =
                                AppState::AddingNewTodo("".into(), "".into(), Some(parent_id));
                            Command::none()
                        }
                    }
                } else {
                    Command::none()
                }
            }
            Message::TodoToggled(result) => {
                if result.is_ok() {
                    let ok_result = result.unwrap();
                    if let Some(todo) =
                        TodoHierarchy::get_hierarchy_by_id(&mut self.todos, ok_result.0)
                    {
                        todo.toggle_with_children(ok_result.1);
                    }
                }
                Command::none()
            }
            Message::ChangeUrl(new_url) => {
                self.settings.server_url = new_url;
                Command::none()
            }
            Message::SaveSettings => {
                self.settings.save();
                self.state = AppState::Loaded("".into());
                Command::none()
            }
            Message::LoadScreenSettings => {
                self.state = AppState::Settings;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        let content: Element<_> = match &self.state {
            AppState::Loading => text("Loading...").into(),
            AppState::Loaded(input_value) => {
                let control_buttons = row![
                    text("Timely").size(28),
                    button("Add new").on_press(Message::LoadScreenAddNewTodo(
                        "".into(),
                        "".into(),
                        None
                    )),
                    button("Refresh").on_press(Message::Load),
                    button("Settings").on_press(Message::LoadScreenSettings)
                ]
                .align_items(Alignment::Center)
                .spacing(18);
                match self.todos.len() {
                    0 => column![control_buttons, text("No todos!")]
                        .spacing(24)
                        .into(),
                    _ => column![
                        control_buttons,
                        scrollable(keyed_column(self.todos.iter().map(|todo| {
                            (
                                todo.todo.id,
                                todo.view().map(move |message| {
                                    Message::TodoMessage(todo.todo.id, message)
                                }),
                            )
                        }))),
                    ]
                    .spacing(24)
                    .into(),
                }
            }
            AppState::Errored(error) => column![
                row![
                    button("Refresh").on_press(Message::Load),
                    button("Settings").on_press(Message::LoadScreenSettings),
                ]
                .align_items(Alignment::Center)
                .spacing(18),
                text(format!("Error: {}", error))
            ]
            .spacing(24)
            .into(),
            AppState::AddingNewTodo(name, description, parent_id) => column![
                button("Go back").on_press(Message::GoBackToMain),
                row![
                    text("Name:"),
                    text_input("Task name", &name).on_input(|new_name| {
                        Message::LoadScreenAddNewTodo(new_name, description.clone(), *parent_id)
                    }),
                ]
                .align_items(Alignment::Center)
                .spacing(10),
                row![
                    text("Description:"),
                    text_input("Task description", &description).on_input(|new_description| {
                        Message::LoadScreenAddNewTodo(name.clone(), new_description, *parent_id)
                    }),
                ]
                .align_items(Alignment::Center)
                .spacing(10),
                button("Submit").on_press(Message::SubmitNewTodo(
                    name.clone(),
                    description.clone(),
                    *parent_id
                ))
            ]
            .spacing(10)
            .into(),
            AppState::Settings => column![
                button("Go back").on_press(Message::GoBackToMain),
                row![
                    text("Server url:"),
                    text_input("Server url", self.settings.server_url.as_str())
                        .on_input(Message::ChangeUrl),
                ]
                .align_items(Alignment::Center)
                .spacing(10),
                button("Save").on_press(Message::SaveSettings)
            ]
            .spacing(10)
            .into(),
        };
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .into()
    }
}

#[derive(Clone, Debug)]
pub struct TodoHierarchy {
    pub todo: Todo,
    pub children: Vec<TodoHierarchy>,
}

pub fn build_hierarchy(todos: Vec<Todo>) -> Vec<TodoHierarchy> {
    let mut todo_map: HashMap<i64, TodoHierarchy> = HashMap::new();
    let mut root_todos: Vec<TodoHierarchy> = Vec::new();

    // Step 1: Create TodoHierarchy for each Todo and store in a HashMap
    for todo in todos {
        let id = todo.id;
        todo_map.insert(
            id,
            TodoHierarchy {
                todo,
                children: Vec::new(),
            },
        );
    }

    // Step 2: Iterate through the map and build the hierarchy
    for (id, hierarchy) in todo_map.clone() {
        if let Some(parent_id) = hierarchy.todo.parent_id {
            if let Some(parent_hierarchy) = todo_map.get_mut(&parent_id) {
                parent_hierarchy.children.push(hierarchy);
                todo_map.remove(&id);
            }
        } else {
            root_todos.push(hierarchy);
        }
    }
    fn sort_hierarchy_by_id(hierarchies: &mut Vec<TodoHierarchy>) {
        hierarchies.sort_by(|a, b| a.todo.id.cmp(&b.todo.id)); // Sort by id

        // Recursively sort children
        for hierarchy in hierarchies {
            sort_hierarchy_by_id(&mut hierarchy.children);
        }
    }

    let mut hierarchy_vec: Vec<TodoHierarchy> = todo_map.into_values().collect();

    sort_hierarchy_by_id(&mut hierarchy_vec);
    hierarchy_vec
}

pub fn add_to_hierarchy(hierarchy: &mut Vec<TodoHierarchy>, todo: Todo) {
    fn try_add_to_hierarchy(hierarchy: &mut Vec<TodoHierarchy>, todo: Todo) -> bool {
        for sub_hierarchy in hierarchy {
            if sub_hierarchy.todo.id == todo.parent_id.unwrap_or(-1) {
                sub_hierarchy.children.push(TodoHierarchy::new(todo));
                return true;
            } else if try_add_to_hierarchy(&mut sub_hierarchy.children, todo.clone()) {
                return true;
            }
        }
        false
    }

    if !try_add_to_hierarchy(hierarchy, todo.clone()) {
        hierarchy.push(TodoHierarchy::new(todo));
    }
}

#[derive(Clone, Debug)]
enum TodoMessage {
    Done(i64, bool),
    Delete(i64),
    AddChild(i64),
}

impl TodoHierarchy {
    pub fn new(todo: Todo) -> TodoHierarchy {
        TodoHierarchy {
            todo,
            children: Vec::new(),
        }
    }

    fn view(&self) -> Element<'_, TodoMessage> {
        let name_and_desc = if let Some(desc) = &self.todo.description {
            if desc != "" {
                column![text(&self.todo.name).size(16), text(desc).size(12)].padding([0, 16, 0, 0])
            } else {
                column![text(&self.todo.name).size(16)].padding([0, 16, 0, 0])
            }
        } else {
            column![text(&self.todo.name).size(16)].padding([0, 16, 0, 0])
        };
        let mut col: Column<TodoMessage> = column![row![
            checkbox("", self.todo.done).on_toggle(|state| TodoMessage::Done(self.todo.id, state)),
            name_and_desc,
            (button(delete_icon()))
                .on_press(TodoMessage::Delete(self.todo.id))
                .width(28)
                .height(28)
                .padding(2),
            button(add_icon())
                .on_press(TodoMessage::AddChild(self.todo.id))
                .width(28)
                .height(28)
                .padding(2)
        ]
        .align_items(Alignment::Center)
        .spacing(8)]
        .spacing(8);

        // Add the children recursively
        for child in &self.children {
            col = col.push(
                Container::new(child.view())
                    .padding([0, 0, 0, 8])
                    .width(Length::Fill),
            );
        }

        Container::new(col).padding(10).width(Length::Fill).into()
    }
    pub fn get_hierarchy_by_id(
        hierarchies: &mut Vec<TodoHierarchy>,
        id: i64,
    ) -> Option<&mut TodoHierarchy> {
        for hierarchy in hierarchies {
            if hierarchy.todo.id == id {
                return Some(hierarchy);
            } else if let Some(found) =
                TodoHierarchy::get_hierarchy_by_id(&mut hierarchy.children, id)
            {
                return Some(found);
            }
        }
        None
    }

    pub fn toggle_with_children(&mut self, state: bool) {
        // Set the current todo's state
        self.todo.done = state;

        // Recursively set the state for all children
        for child in &mut self.children {
            child.toggle_with_children(state);
        }
    }
}

fn main() -> iced::Result {
    let settings = AppSettings::load().unwrap_or(AppSettings {
        server_url: "http://localhost:3000".into(),
        palette: "LIGHT".into(),
    });

    println!("Using server url: {}", settings.server_url);

    App::run(Settings {
        antialiasing: true,
        flags: settings,
        default_font: Font::DEFAULT,
        default_text_size: Pixels(16.),
        id: None,
        window: iced::window::Settings {
            size: Size {
                width: 800.,
                height: 350.,
            },
            position: iced::window::Position::Centered,
            min_size: Some(Size {
                width: 150.,
                height: 150.,
            }),
            ..Default::default()
        },
        fonts: vec![],
    })
}
