use axum::{routing::get, Router};

use crate::{database::init_db, tasks::update_guilds};

mod gw2api;
mod data;
mod database;
mod tasks;


#[tokio::main]
async fn main() {


    let pool = init_db().await.unwrap();
    update_guilds(&pool).await;

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}