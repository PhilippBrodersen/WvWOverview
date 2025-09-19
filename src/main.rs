#![warn(clippy::pedantic)]

use axum::{Router, routing::get};

use crate::{
    database::init_db,
    tasks::{run_guild_updater, run_match_updater, update_guilds},
};

mod data;
mod database;
mod gw2api;
mod tasks;

#[tokio::main]
async fn main() {
    let pool = init_db().await.unwrap();
    run_guild_updater(&pool).await;
    run_match_updater(&pool).await;

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
