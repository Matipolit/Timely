use axum::{
    extract::{self, Form, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use dotenvy::dotenv;
use serde::Deserialize;
use sha2::{
    digest::{
        generic_array::GenericArray,
        typenum::{
            bit::{B0, B1},
            UInt, UTerm,
        },
    },
    Digest, Sha256,
};
use sqlx::postgres::PgPool;
use std::env;
use time::{self, Date, Month};
use timely_lib::{build_hierarchy, month_num_to_month, Done, Todo};
use tower_http::trace::{
    DefaultMakeSpan, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer,
};

use tera::Tera;

use tracing::Level;
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    hashed_password: DigestedHash,
    templates: Tera,
    running_on_subpath: bool,
}

#[derive(Deserialize)]
struct CreateTodo {
    name: String,
    description: Option<String>,
    parent_id: Option<i64>,
    date: Option<String>,
}

#[derive(Deserialize)]
struct PasswordQuery {
    password: Option<String>,
}

#[derive(Deserialize)]
struct DateQuery {
    date_less: Option<Date>,
    date_more: Option<Date>,
}

// For the login form (from the web UI)
#[derive(Deserialize)]
struct LoginForm {
    password: String,
}

type DigestedHash =
    GenericArray<u8, UInt<UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>, B0>>;

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .init();
    }
    dotenv().expect(".env file not found");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let service_url = env::var("SERVICE_URL").expect("SERVICE_URL not set");
    let password = env::var("PASSWORD").expect("PASSWORD not set");

    // Compute the hash of the password (we use this both for API and web authentication)
    let hashed_password = Sha256::digest(password);

    println!("Using database url: {}", &database_url);
    let pool = PgPool::connect(&database_url).await.unwrap();

    // Initialize Tera – assuming your templates are in a folder named "templates"
    let templates = Tera::new("templates/**/*").expect("Error initializing Tera");

    let run_on_subpath_env = env::var("RUN_ON_SUBPATH");

    let run_on_subpath = run_on_subpath_env.is_ok_and(|run| run.to_lowercase() == "true");

    let app_state = AppState {
        pool,
        hashed_password,
        templates,
        running_on_subpath: run_on_subpath,
    };

    // Build the app with both web and API routes.
    let app = Router::new()
        // Web UI: the index now renders a Tera template.
        .route("/", get(web_index))
        .route("/login", post(login))
        .route("/logout", get(logout))
        // API endpoints:
        .route(
            "/todos",
            get(get_todos).post(create_todo).delete(delete_todo),
        )
        .route("/todos/toggle", post(toggle_todo))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO)) // Log requests
                .on_request(DefaultOnRequest::new().level(Level::INFO)) // Log request details
                .on_response(DefaultOnResponse::new().level(Level::INFO)) // Log response details
                .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
        )
        .with_state(app_state);
    let listener = tokio::net::TcpListener::bind(&service_url).await.unwrap();
    if run_on_subpath {
        let subpath_router = Router::new().nest("/timely", app);
        println!("Listening on http://{}/timely", service_url);
        axum::serve(listener, subpath_router).await.unwrap()
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

/// Helper for API endpoints: extract a provided password from either the query or a cookie.
fn extract_provided(query: &PasswordQuery, cookies: &CookieJar) -> Option<String> {
    query
        .password
        .as_ref()
        .and_then(|p| Some(p.clone()))
        .or_else(|| cookies.get("auth").map(|c| c.value().to_owned()))
}

/// API: Get all todos.
async fn get_todos(
    Query(query): Query<PasswordQuery>,
    Query(date_query): Query<DateQuery>,
    cookies: CookieJar,
    State(state): State<AppState>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    println!("getting todos");
    let provided = extract_provided(&query, &cookies);
    if provided.is_some_and(|p| authenticate(&state.hashed_password, &p)) {
        println!("getting todos inner");
        get_todos_json_inner(&state.pool, date_query.date_less, date_query.date_more).await
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

async fn get_todos_inner(
    pool: &PgPool,
    date_less: Option<Date>,
    date_more: Option<Date>,
) -> Result<Vec<Todo>, (StatusCode, String)> {
    let todos = if date_less.is_none() && date_more.is_none() {
        sqlx::query_as!(
            Todo,
            r#"
                        SELECT id, name, done, description, parent_id, date
                        FROM todos
                        ORDER BY id
                    "#
        )
        .fetch_all(pool)
        .await
    } else if date_less.is_some() {
        sqlx::query_as!(
            Todo,
            r#"
                        SELECT id, name, done, description, parent_id, date
                        FROM todos
                        WHERE date <= $1
                        ORDER BY id
                    "#,
            date_less.unwrap()
        )
        .fetch_all(pool)
        .await
    } else if date_more.is_some() {
        sqlx::query_as!(
            Todo,
            r#"
                        SELECT id, name, done, description, parent_id, date
                        FROM todos
                        WHERE date >= $1
                        ORDER BY id
                    "#,
            date_more.unwrap()
        )
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as!(
            Todo,
            r#"
                        SELECT id, name, done, description, parent_id, date
                        FROM todos
                        WHERE date BETWEEN $1 AND $2
                        ORDER BY id
                    "#,
            date_more.unwrap(),
            date_less.unwrap()
        )
        .fetch_all(pool)
        .await
    };
    match todos {
        Ok(todos_vec) => Ok(todos_vec),
        Err(err) => Err(internal_error(err)),
    }
}
/// API: Helper function to get todos.
async fn get_todos_json_inner(
    pool: &PgPool,
    date_less: Option<Date>,
    date_more: Option<Date>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let todos = get_todos_inner(pool, date_less, date_more).await;
    match todos {
        Ok(todos_vec) => Ok(Json(todos_vec)),
        Err(err) => Err(err),
    }
}

/// API: Create a new todo.
async fn create_todo(
    query: Query<PasswordQuery>,
    cookies: CookieJar,
    State(state): State<AppState>,
    extract::Json(payload): extract::Json<CreateTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    println!("creating todo!");
    let provided = extract_provided(&query, &cookies);
    let date_from_payload_opt = payload.date;
    let converted_date: Option<Date> = if let Some(date_from_payload) = date_from_payload_opt {
        let trimmed_date = date_from_payload.trim().to_owned();
        if trimmed_date != "" {
            tracing::debug!("Date is: {}", &trimmed_date);
            let mut split_date = trimmed_date.split("-");
            let year: i32 = split_date.next().unwrap().parse().unwrap();
            let month: Option<Month> =
                month_num_to_month(split_date.next().unwrap().parse().unwrap());
            let day: u8 = split_date.next().unwrap().parse().unwrap();
            Some(Date::from_calendar_date(year, month.unwrap(), day).unwrap())
        } else {
            None
        }
    } else {
        tracing::debug!("Date is none");
        None
    };
    if provided.is_some_and(|p| authenticate(&state.hashed_password, &p)) {
        let new_todo = sqlx::query_as!(
            Todo,
            r#"
            INSERT INTO todos (name, description, parent_id, date)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, done, description, parent_id, date
            "#,
            payload.name,
            payload.description,
            payload.parent_id,
            converted_date
        )
        .fetch_one(&state.pool)
        .await;

        match new_todo {
            Ok(record) => Ok(Json(record)),
            Err(err) => Err(internal_error(err)),
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

/// API: Delete a todo (and its descendants).
async fn delete_todo(
    Query(query): Query<PasswordQuery>,
    Query(date_query): Query<DateQuery>,
    cookies: CookieJar,
    State(state): State<AppState>,
    extract::Json(id_to_delete): extract::Json<i64>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let provided = extract_provided(&query, &cookies);
    if provided.is_some_and(|p| authenticate(&state.hashed_password, &p)) {
        // 1. Fetch the todo to delete (ensure it exists)
        let todo_to_delete =
            sqlx::query_as!(Todo, "SELECT * FROM todos WHERE id = $1", id_to_delete)
                .fetch_one(&state.pool)
                .await
                .map_err(|e| internal_error(e))?;

        // 2. Use a recursive CTE to delete the todo and all its descendants.
        let delete_successful = sqlx::query!(
            r#"
            WITH RECURSIVE todo_hierarchy AS (
                SELECT id FROM todos WHERE id = $1
                UNION
                SELECT t.id FROM todos t
                INNER JOIN todo_hierarchy th ON t.parent_id = th.id
            )
            DELETE FROM todos WHERE id IN (SELECT id FROM todo_hierarchy);
            "#,
            todo_to_delete.id
        )
        .execute(&state.pool)
        .await
        .map(|res| res.rows_affected() > 0)
        .unwrap_or(false);

        // 3. Fetch updated todo list after deletion.
        let new_todos =
            get_todos_json_inner(&state.pool, date_query.date_less, date_query.date_more)
                .await
                .unwrap();

        if delete_successful {
            Ok(new_todos)
        } else {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not delete".to_owned(),
            ))
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

/// API: Toggle a todo (and its children).
async fn toggle_todo(
    Query(query): Query<PasswordQuery>,
    cookies: CookieJar,
    State(state): State<AppState>,
    extract::Json(todo_id): extract::Json<i64>,
) -> Result<Json<bool>, (StatusCode, String)> {
    let provided = extract_provided(&query, &cookies);
    if provided.is_some_and(|p| authenticate(&state.hashed_password, &p)) {
        let toggle_result = sqlx::query_as!(
            Done,
            r#"
            WITH RECURSIVE updated_parent AS (
                -- Toggle parent's state and return the new value
                UPDATE todos
                SET done = NOT done
                WHERE id = $1
                RETURNING done
            ),
            todo_hierarchy AS (
                -- Recursively select all children (and grandchildren, etc.)
                SELECT id FROM todos WHERE parent_id = $1
                UNION ALL
                SELECT t.id
                FROM todos t
                INNER JOIN todo_hierarchy th ON t.parent_id = th.id
            ),
            updated_children AS (
                -- Update all descendants to match parent's new state
                UPDATE todos
                SET done = (SELECT done FROM updated_parent)
                WHERE id IN (SELECT id FROM todo_hierarchy)
                RETURNING id
            )
            -- Return the parent's new done state.
            SELECT done FROM updated_parent;
            "#,
            todo_id
        )
        .fetch_one(&state.pool)
        .await;

        match toggle_result {
            Ok(done) => Ok(Json(done.done)),
            Err(err) => Err(internal_error(err)),
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, "Failed authentication".to_owned()))
    }
}

/// Helper to map internal errors.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

/// -----------------
/// Web Handlers
/// -----------------

/// GET "/" – renders the web interface. If the user is not authenticated,
/// the page shows a login form. If authenticated, it shows the todo UI.
async fn web_index(
    cookies: CookieJar,
    State(state): State<AppState>,

    Query(date_query): Query<DateQuery>,
) -> impl IntoResponse {
    let auth_cookie = cookies.get("auth").map(|cookie| cookie.value().to_owned());
    let is_auth = if auth_cookie.is_some() {
        authenticate(&state.hashed_password, &auth_cookie.unwrap())
    } else {
        false
    };
    let mut context = tera::Context::new();
    let todos = get_todos_inner(&state.pool, date_query.date_less, date_query.date_more).await;
    if let Ok(ok_todos) = todos {
        let hierarchy = build_hierarchy(ok_todos);
        context.insert("todos", &hierarchy);
    }
    context.insert("authenticated", &is_auth);
    context.insert("subpath", &state.running_on_subpath);
    // You can also pass additional variables as needed.
    let rendered = state
        .templates
        .render("index.html", &context)
        .unwrap_or_else(|err| format!("Template error: {}", err));
    Html(rendered)
}

/// POST "/login" – processes the login form. If the password is correct,
/// it sets a cookie (with the hashed password in hex) and redirects to "/".
async fn login(
    cookies: CookieJar,
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let redirect = if state.running_on_subpath {
        Redirect::to("/timely")
    } else {
        Redirect::to("/")
    };
    if authenticate(&state.hashed_password, &form.password) {
        let cookie = Cookie::build(("auth", form.password))
            .path("/")
            // For web UI usage you may want JS to read it, so not HTTP-only.
            .http_only(false);

        let cookies = cookies.add(cookie);
        (cookies, redirect)
    } else {
        // On failed login, simply redirect back.
        (cookies, redirect)
    }
}

/// GET "/logout" – clears the auth cookie and redirects to "/".
async fn logout(cookies: CookieJar, State(state): State<AppState>) -> impl IntoResponse {
    let cookie = Cookie::build(("auth", ""))
        .path("/")
        // Set cookie to expire immediately.
        .max_age(time::Duration::seconds(0));
    let cookies = cookies.remove(cookie);
    let redirect = if state.running_on_subpath {
        Redirect::to("/timely")
    } else {
        Redirect::to("/")
    };
    (cookies, redirect)
}
