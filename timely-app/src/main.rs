use config::{Config, ConfigError, File};
use iced::theme::Palette;
use iced::widget::{
    button, checkbox, column, container, keyed_column, row, scrollable, text, text_input, Column,
    Container, Text,
};
use iced::{
    alignment, font, Alignment, Application, Element, Font, Length, Pixels, Settings, Size, Task,
    Theme,
};
use indexmap::IndexMap;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::env::home_dir;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;

use timely_lib::{build_hierarchy, Todo, TodoHierarchy};

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
    map.insert("gruvbox_dark", Palette::GRUVBOX_DARK);
    map.insert("gruvbox_light", Palette::GRUVBOX_LIGHT);
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
    password: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            server_url: "http://localhost:3000".into(),
            palette: "light".into(),
            password: "123".into(),
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
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
}

fn edit_icon() -> Text<'static> {
    icon('\u{F303}')
}

fn add_icon() -> Text<'static> {
    text("+")
        .size(16)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
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
    ChangePassword(String),
    SaveSettings,
    None,
}

async fn load(client: Client, url: String, password: String) -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = client
        .get(format!("{}/todos?password={}", url, password))
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
    password: String,
) -> Result<Todo, Error> {
    let response: Todo = client
        .post(format!("{}/todos?password={}", url, password))
        .json(&todo_to_send)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn delete_todo(
    id: i64,
    client: Client,
    url: String,
    password: String,
) -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = client
        .delete(format!("{}/todos?password={}", url, password))
        .json(&id)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn toggle_todo(
    id: i64,
    client: Client,
    url: String,
    password: String,
) -> Result<(i64, bool), Error> {
    let response: bool = client
        .post(format!("{}/todos/toggle?password={}", url, password))
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

impl App {
    fn new(server_url: String, password: String, palette: String) -> (Self, Task<Message>) {
        let client = Client::new();
        let command = Task::batch([
            font::load(include_bytes!("../fonts/icons.ttf").as_slice()).map(Message::FontLoaded),
            Task::perform(
                load(client.clone(), server_url.clone(), password.clone()),
                Message::Loaded,
            ),
        ]);

        let app = App {
            state: AppState::Loading,
            todos: Vec::new(),
            client,
            palette: *palette_map()
                .get(palette.as_str())
                .unwrap_or(&Palette::LIGHT),
            settings: AppSettings {
                server_url,
                palette,
                password,
            },
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

    fn theme(&self) -> Theme {
        Theme::custom("user_theme".into(), self.palette)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(todos_result) => match todos_result {
                Ok(todos) => {
                    let hierarchy = build_hierarchy(todos);
                    self.state = AppState::Loaded("".to_owned());
                    self.todos = hierarchy;
                    Task::none()
                }
                Err(todos_error) => {
                    self.state = AppState::Errored(format!("{:?}", todos_error));
                    Task::none()
                }
            },
            Message::Load => Task::perform(
                load(
                    self.client.clone(),
                    self.settings.server_url.clone(),
                    self.settings.password.clone(),
                ),
                Message::Loaded,
            ),
            Message::None => Task::none(),
            Message::LoadScreenAddNewTodo(title, description, parent_id) => {
                self.state = AppState::AddingNewTodo(title, description, parent_id);
                Task::none()
            }
            Message::GoBackToMain => {
                self.state = AppState::Loaded("".to_owned());
                Task::none()
            }
            Message::SubmitNewTodo(name, description, parent_id) => Task::perform(
                submit_new_todo(
                    TodoToSend {
                        name,
                        description,
                        parent_id,
                    },
                    self.client.clone(),
                    self.settings.server_url.clone(),
                    self.settings.password.clone(),
                ),
                Message::SubmittedNewTodo,
            ),
            Message::SubmittedNewTodo(todo) => {
                if todo.is_ok() {
                    add_to_hierarchy(&mut self.todos, todo.unwrap());
                    self.state = AppState::Loaded("".to_owned());
                }
                Task::none()
            }
            Message::FontLoaded(_) => Task::none(),
            Message::TodoMessage(id, message) => {
                if let Some(_todo) = TodoHierarchy::get_hierarchy_by_id(&mut self.todos, id) {
                    match message {
                        TodoMessage::Done(id, _state) => Task::perform(
                            toggle_todo(
                                id,
                                self.client.clone(),
                                self.settings.server_url.clone(),
                                self.settings.password.clone(),
                            ),
                            Message::TodoToggled,
                        ),
                        TodoMessage::Delete(id) => Task::perform(
                            delete_todo(
                                id,
                                self.client.clone(),
                                self.settings.server_url.clone(),
                                self.settings.password.clone(),
                            ),
                            Message::Loaded,
                        ),
                        TodoMessage::AddChild(parent_id) => {
                            self.state =
                                AppState::AddingNewTodo("".into(), "".into(), Some(parent_id));
                            Task::none()
                        }
                    }
                } else {
                    Task::none()
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
                Task::none()
            }
            Message::ChangeUrl(new_url) => {
                self.settings.server_url = new_url;
                Task::none()
            }
            Message::ChangePassword(new_password) => {
                self.settings.password = new_password;
                Task::none()
            }
            Message::SaveSettings => {
                self.settings.save();
                self.state = AppState::Loaded("".into());
                Task::none()
            }
            Message::LoadScreenSettings => {
                self.state = AppState::Settings;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message, Theme, iced::Renderer> {
        let content: Element<_> = match &self.state {
            AppState::Loading => text("Loading...").into(),
            AppState::Loaded(_input_value) => {
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
                .align_y(Alignment::Center)
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
                                hierarchy_view(todo).map(move |message| {
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
                .align_y(Alignment::Center)
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
                .align_y(Alignment::Center)
                .spacing(10),
                row![
                    text("Description:"),
                    text_input("Task description", &description).on_input(|new_description| {
                        Message::LoadScreenAddNewTodo(name.clone(), new_description, *parent_id)
                    }),
                ]
                .align_y(Alignment::Center)
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
                .align_y(Alignment::Center)
                .spacing(10),
                row![
                    text("Password:"),
                    text_input("Password", self.settings.password.as_str())
                        .on_input(Message::ChangePassword),
                ]
                .align_y(Alignment::Center)
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

fn hierarchy_view(hierarchy: &TodoHierarchy) -> Element<'_, TodoMessage> {
    let name_and_desc = if let Some(desc) = &hierarchy.todo.description {
        if desc != "" {
            column![text(&hierarchy.todo.name).size(16), text(desc).size(12)].padding([0, 16])
        } else {
            column![text(&hierarchy.todo.name).size(16)].padding([0, 16])
        }
    } else {
        column![text(&hierarchy.todo.name).size(16)].padding([0, 16])
    };
    let mut col: Column<TodoMessage> = column![row![
        checkbox("", hierarchy.todo.done)
            .on_toggle(|state| TodoMessage::Done(hierarchy.todo.id, state)),
        name_and_desc,
        (button(delete_icon()))
            .on_press(TodoMessage::Delete(hierarchy.todo.id))
            .width(28)
            .height(28)
            .padding(2),
        button(add_icon())
            .on_press(TodoMessage::AddChild(hierarchy.todo.id))
            .width(28)
            .height(28)
            .padding(2)
    ]
    .align_y(Alignment::Center)
    .spacing(8)]
    .spacing(8);

    // Add the children recursively
    for child in &hierarchy.children {
        col = col.push(
            Container::new(hierarchy_view(child))
                .padding([0, 8])
                .width(Length::Fill),
        );
    }

    Container::new(col).padding(10).width(Length::Fill).into()
}

fn main() -> iced::Result {
    let settings = AppSettings::load().unwrap_or(AppSettings {
        ..Default::default()
    });

    println!("Using server url: {}", settings.server_url);
    println!("Using palette: {}", settings.palette);

    iced::application(App::title, App::update, App::view)
        .default_font(Font::DEFAULT)
        .window_size(Size {
            width: 800.,
            height: 350.,
        })
        .theme(App::theme)
        .position(iced::window::Position::Centered)
        .antialiasing(true)
        .run_with(|| App::new(settings.server_url, settings.password, settings.palette))
}
