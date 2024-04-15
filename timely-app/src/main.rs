use iced::widget::{
    button, checkbox, column, container, horizontal_space, radio, row, scrollable, slider, text,
    text_input, toggler, vertical_space,
};
use iced::{Application, Color, Command, Element, Font, Length, Pixels, Settings, Size, Theme};
use log::{info, Level};
use reqwest;
use serde::Deserialize;

const API_URL: &str = "http://localhost:3000/todos";

use timely_lib::Todo;

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
    Loaded(Vec<Todo>),
    Errored,
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Vec<Todo>, Error>),
    Load,
    TodoToggled(i32),
    TodoDeleted(i32),
    None,
}

async fn load() -> Result<Vec<Todo>, Error> {
    let response: Vec<Todo> = reqwest::Client::new()
        .get(API_URL)
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

#[derive(Debug)]
struct App {
    state: AppState,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        return (
            App {
                state: AppState::Loading,
            },
            Command::perform(load(), Message::Loaded),
        );
    }

    fn title(&self) -> String {
        let subtitle = match self.state {
            AppState::Loading => "Loading - ",
            AppState::Loaded(_) => "",
            AppState::Errored { .. } => "Error - ",
        };

        format!("{subtitle}Timely")
    }
    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::Loaded(todos_result) => match todos_result {
                Ok(todos) => Command::none(),
                Err(todos) => {
                    self.state = AppState::Errored;
                    return Command::none();
                }
            },
            Message::Load => Command::perform(load(), Message::Loaded),
            Message::TodoToggled(todo_id) => todo!(),
            Message::TodoDeleted(todo_id) => todo!(),
            Message::None => Command::none(),
        }
    }

    fn view(&self) -> Element<'_, Self::Message, Self::Theme, iced::Renderer> {
        todo!()
    }
}

fn main() -> iced::Result {
    App::run(Settings {
        antialiasing: true,
        ..Default::default()
    })
}
