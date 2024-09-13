use std::env;

use axum::{
    extract::{self, Query, State},
    http::StatusCode,
    response::Html,
    routing::{delete, get, post},
    Json, Router,
};

use serde::Deserialize;

use dotenvy::dotenv;
use sha2::{
    digest::{
        consts::{B0, B1},
        generic_array::GenericArray,
        typenum::{UInt, UTerm},
    },
    Digest, Sha256,
};
use sqlx::postgres::PgPool;
use timely_lib::Todo;

#[derive(Deserialize)]
struct CreateTodo {
    name: String,
    description: Option<String>,
    parent_id: Option<i64>,
}

#[derive(Deserialize)]
struct PasswordQuery {
    password: String,
}

type DigestedHash =
    GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>;

#[tokio::main]
async fn main() {
    dotenv().expect(".env file not found");

    let database_url = &env::var("DATABASE_URL").unwrap();
    let service_url = &env::var("SERVICE_URL").unwrap();
    let password = &env::var("PASSWORD").unwrap();

    let hashed_password = Sha256::digest(password);

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
        .with_state((pool,hashed_password));

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

fn authenticate(original_hash: &DigestedHash, provided_pass: &String) -> bool {
    if Sha256::digest(provided_pass).eq(original_hash) {
        return true;
    } else {
        return false;
    }
}

async fn get_todos(
    Query(query): Query<PasswordQuery>,
    State((pool, hash)): State<(PgPool, DigestedHash)>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    if authenticate(&hash, &query.password) {
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
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

async fn create_todo(
    Query(query): Query<PasswordQuery>,
    State((pool, hash)): State<(PgPool, DigestedHash)>,
    extract::Json(payload): extract::Json<CreateTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    if authenticate(&hash, &query.password) {
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
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

async fn delete_todo(
    Query(query): Query<PasswordQuery>,
    State((pool, hash)): State<(PgPool, DigestedHash)>,
    extract::Json(id_to_delete): extract::Json<i64>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    if authenticate(&hash, &query.password) {
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

        let new_todos = get_todos(Query(query), State((pool, hash))).await.unwrap();
        match delete_successful {
            true => return Ok(new_todos),
            false => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Could not delete".to_owned(),
                ))
            }
        };
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

async fn toggle_todo(
    Query(query): Query<PasswordQuery>,
    State((pool, hash)): State<(PgPool, DigestedHash)>,
    extract::Json(todo_id): extract::Json<i64>,
) -> Result<Json<bool>, (StatusCode, String)> {
    if authenticate(&hash, &query.password) {
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
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
