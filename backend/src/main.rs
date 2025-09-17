#![warn(clippy::pedantic)]



use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

mod gw2api;
mod processing;
mod database;

#[tokio::main]
async fn main() -> Result<(), ()> {
    // initialize tracing
    /* tracing_subscriber::fmt::init();

    let db_url = "sqlite://mydb.sqlite";

    // Setup step: handle errors explicitly
    let pool = match SqlitePool::connect(db_url).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("Failed to connect to database: {e}");
            return Err(e.into());
        }
    };

    if let Err(e) = init_db(&pool).await {
        eprintln!("Failed to initialize database: {e}");
        return Err(e.into());
    }

    // Later steps: don't crash, just log errors
    if let Err(e) = add_test_entry(&pool, "Alice").await {
        eprintln!("Could not add test entry: {e}");
        return Err(e.into());
    }

    Ok(()) */

    /*  // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(()) */
    Ok(())
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS test (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_test_entry(pool: &SqlitePool, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO test (name)
        VALUES (?1)
        "#,
    )
    .bind(name)
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_user(
    // this argument tells axum to parse the request body
    // as JSON into a `CreateUser` type
    Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
    // insert your application logic here
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // this will be converted into a JSON response
    // with a status code of `201 Created`
    (StatusCode::CREATED, Json(user))
}

// the input to our `create_user` handler
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// the output to our `create_user` handler
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}
