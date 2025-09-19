#![warn(clippy::pedantic)]

use std::sync::Arc;

use axum::{extract::State, response::{Html, IntoResponse}, routing::get, Json, Router};
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;


use crate::{
    data::{Data}, database::init_db, tasks::{build_all_matches, run_guild_updater, run_match_updater, run_mateches_cache_updater}
};

mod data;
mod database;
mod gw2api;
mod tasks;

const INDEX_HTML: &str = include_str!("../static/frontend/index.html");
const FAVICON_SVG: &str = include_str!("../static/frontend/favicons/swords.svg");



#[tokio::main]
async fn main() {

    let frontend_service = ServeDir::new("static/frontend");
    let pool = init_db().await.unwrap();

    run_guild_updater(&pool).await;
    //run_match_updater(&pool).await;
    let cache: Arc<RwLock<Data>> = Arc::new(RwLock::new(Data::default()));
    run_mateches_cache_updater(&pool, cache.clone()).await;

    let root_route: Router<()> = Router::new()
        .route("/", get(index)).layer(CompressionLayer::new());

    let data_route: Router<()> = Router::new()
        .route("/data/", get(data))
        .with_state(cache)
        .layer(CompressionLayer::new());

    let favicon_route: Router<()> = Router::new()
        .route("/favicon.svg", get(favicon))
        .route("/favicon.ico", get(favicon)) 
        .layer(CompressionLayer::new());



    let app = Router::new()
        .merge(root_route)
        .merge(data_route)
        .merge(favicon_route);
        //.nest_service("/", frontend_service);

    /* // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root)).with_state(pool).route("/test/", get(data)).with_state(cache); //layer(CompressionLayer::new()); */

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        FAVICON_SVG,
    )
}


async fn data(State(cache): State< Arc<RwLock<Data>>>) -> Json<Data> {
    let read_guard = cache.read().await;
    let cloned = read_guard.clone();
    Json(cloned)
}
