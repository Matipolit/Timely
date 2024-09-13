use std::env;

use axum::{
    extract::{self, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};

use serde::Deserialize;

use dotenvy::dotenv;
use sqlx::postgres::PgPool;
use timely_lib::Todo;

#[derive(Deserialize)]
struct CreateTodo {
    name: String,
    description: Option<String>,
    parent_id: Option<i64>,
}

#[tokio::main]
async fn main() {
    dotenv().expect(".env file not found");

    let database_url = &env::var("DATABASE_URL").unwrap();
    let service_url = &env::var("SERVICE_URL").unwrap();
    println!("Using database url: {}", &database_url);
    let pool = PgPool::connect(database_url).await.unwrap();
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                "Welcome to Timely!
GET /todos - get all todos: Vec<Todo>
POST /todos, body: [{name: string, description: option<string>, parent_id: option<int>}] - create todo : Todo
POST /todos/toggle, body: [{id: int}] - toggle todo: Bool
DELETE /todos, body: [{id: int}]- delete todo: Vec<Todo>"
            }),
        )
        .route("/todos", get(get_todos))
        .route("/todos", post(create_todo))
        .route("/todos/toggle", post(toggle_todo))
        .route("/todos", delete(delete_todo))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind(service_url).await.unwrap();
    let run_on_subpath = &env::var("RUN_ON_SUBPATH");
    if run_on_subpath.is_ok() {
        if run_on_subpath.as_ref().unwrap().to_lowercase() == "true" {
            let subpath_router = Router::new().nest("/timely", app);
            println!("Listening on http://{}/timely", service_url);
            axum::serve(listener, subpath_router).await.unwrap()
        } else {
            println!("Listening on http://{}", service_url);
            axum::serve(listener, app).await.unwrap()
        }
    } else {
        println!("Listening on http://{}", service_url);
        axum::serve(listener, app).await.unwrap()
    }
}

async fn get_todos(State(pool): State<PgPool>) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let todos = sqlx::query_as!(
        Todo,
        "
            SELECT id, name, done, description, parent_id 
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
            INSERT INTO todos (name, description, parent_id )
            VALUES ( $1, $2, $3)
            RETURNING id, name, done, description, parent_id
        ",
        payload.name,
        payload.description,
        payload.parent_id
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
    let todo_to_delete = sqlx::query_as!(
        Todo,
        "
            SELECT * FROM todos
            WHERE id = $1
        ",
        id_to_delete
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    //delete all children of the todo
    let _delete_children_successful = sqlx::query!(
        "DELETE FROM todos
        WHERE parent_id = $1",
        todo_to_delete.id
    )
    .execute(&pool)
    .await
    .unwrap();
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
) -> Result<Json<bool>, (StatusCode, String)> {
    //toggle task and its children
    let toggle_result = sqlx::query_as!(
        Todo,
        "
            UPDATE todos
            SET done = NOT done
            WHERE id = $1 OR parent_id = $1
            RETURNING id, name, done, description, parent_id
        ",
        todo_id
    )
    .fetch_one(&pool)
    .await;

    match toggle_result {
        Ok(todo) => Ok(Json(todo.done)),
        Err(err) => Err(internal_error(err)),
    }
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
