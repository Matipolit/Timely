use std::env;

use axum::{
    extract::{self, FromRef, FromRequestParts, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
    Json, Router,
};
use axum_template::{engine::Engine, Key, RenderHtml};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tera::Tera;
use uuid::Uuid;

use anyhow;
use dotenvy::dotenv;
use sqlx::{postgres::PgPool, Acquire};
use timely_lib::Todo;

type AppEngine = Engine<Tera>;

#[derive(Deserialize)]
struct CreateTodo {
    name: String,
    description: Option<String>,
}

const URL: &str = "localhost:3000";

#[tokio::main]
async fn main() {
    dotenv().expect(".env file not found");

    let database_url = &env::var("DATABASE_URL").unwrap();
    println!("Using database url: {}", &database_url);
    let pool = PgPool::connect(database_url).await.unwrap();
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                "Welcome to Timely!
GET /todos - get all todos
POST /todos, body: [{name: string, description: option<string>}] - create todo
POST /todos/toggle, body: [{id: int}] - toggle todo
DELETE /todos, body: [{id: int}]- delete todo"
            }),
        )
        .route("/todos", get(get_todos))
        .route("/todos", post(create_todo))
        .route("/todos/toggle", post(toggle_todo))
        .route("/todos", delete(delete_todo))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind(URL).await.unwrap();
    println!("Listening on http://{}", URL);
    axum::serve(listener, app).await.unwrap();
}

async fn get_todos(State(pool): State<PgPool>) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let todos = sqlx::query_as!(
        Todo,
        "
            SELECT id, name, done, description 
            FROM todos
            ORDER BY id
        "
    )
    .fetch_all(&pool)
    .await;

    match todos {
        Ok(todos_vec) => return Ok(Json(todos_vec)),
        Err(err) => return Err(internal_error(err)),
    };
}

async fn create_todo(
    State(pool): State<PgPool>,
    extract::Json(payload): extract::Json<CreateTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let new_todo = sqlx::query_as!(
        Todo,
        "
            INSERT INTO todos (name, description )
            VALUES ( $1, $2)
            RETURNING id, name, done, description
        ",
        payload.name,
        payload.description,
    )
    .fetch_one(&pool)
    .await;

    match new_todo {
        Ok(record) => return Ok(Json(record)),
        Err(err) => return Err(internal_error(err)),
    };
}

async fn delete_todo(
    State(pool): State<PgPool>,
    extract::Json(id_to_delete): extract::Json<i64>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let delete_successful = sqlx::query!(
        "DELETE FROM todos
        WHERE id = $1",
        id_to_delete
    )
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected()
        > 0;

    let new_todos = get_todos(State(pool)).await.unwrap();
    match delete_successful {
        true => return Ok(new_todos),
        false => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not delete".to_owned(),
            ))
        }
    };
}

async fn toggle_todo(
    State(pool): State<PgPool>,
    extract::Json(todo_id): extract::Json<i64>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let toggle_result = sqlx::query_as!(
        Todo,
        "
            UPDATE todos
            SET done = NOT done
            WHERE id = $1
            RETURNING id, name, done, description
        ",
        todo_id
    )
    .fetch_one(&pool)
    .await;

    match toggle_result {
        Ok(todo) => Ok(Json(todo)),
        Err(err) => Err(internal_error(err)),
    }
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
