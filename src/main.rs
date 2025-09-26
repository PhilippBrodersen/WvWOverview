#![warn(clippy::pedantic)]

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, Response},
    response::{Html, IntoResponse},
    routing::get,
};
use reqwest::StatusCode;
use tokio::sync::RwLock;
use tower_http::compression::CompressionLayer;

use crate::{
    data::Data,
    database::init_db,
    rate_limiter::ApiQueue,
    tasks::{run_mateches_cache_updater, start_update_loops},
};
use clap::{Parser, command};

mod data;
mod database;
mod gw2api;
mod rate_limiter;
mod tasks;

const INDEX_HTML: &str = include_str!("../static/frontend/index.html");
const FAVICON_SVG: &str = include_str!("../static/frontend/favicons/swords.svg");

#[derive(Parser, Debug)]
#[command(name = "WvW Overview")]
#[command(about = "A gw2 WvW backend + frontend to view data from the gw2 api", long_about = None)]
struct Args {
    /// IP address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    ip: String,

    /// Port to bind to
    #[arg(long, default_value = "12345")]
    port: u16,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let addr = format!("{}:{}", args.ip, args.port);
    let pool = init_db().await.unwrap();
    let api_queue = Arc::new(ApiQueue::new(Duration::from_millis(210)));

    start_update_loops(&pool, &api_queue);

    let cache: Arc<RwLock<Data>> = Arc::new(RwLock::new(Data::default()));
    run_mateches_cache_updater(&pool, cache.clone()).await;

    let compression = CompressionLayer::new()
        .gzip(true)
        .br(true)
        .deflate(true)
        .zstd(true);

    let root_route: Router<()> = Router::new()
        .route("/", get(index))
        .layer(compression.clone());

    let data_route: Router<()> = Router::new()
        .route("/data/", get(data))
        .with_state(cache.clone())
        .layer(compression.clone());

    let favicon_route: Router<()> = Router::new()
        .route("/favicon.svg", get(favicon))
        .route("/favicon.ico", get(favicon))
        .layer(compression.clone());

    let app = Router::new()
        .merge(root_route)
        .merge(data_route)
        .merge(favicon_route);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        FAVICON_SVG,
    )
}

async fn data(State(cache): State<Arc<RwLock<Data>>>, req: Request<Body>) -> impl IntoResponse {
    if req.headers().get("test").is_none() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    }

    let cloned = cache.read().await.clone();

    let mut hasher = DefaultHasher::new();
    cloned.hash(&mut hasher);
    let etag = format!("\"{:x}\"", hasher.finish());

    if let Some(if_none_match) = req.headers().get("if-none-match")
        && if_none_match.to_str().unwrap_or("") == etag
    {
        // Data hasn't changed, return 304
        return Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap();
    }

    let json = serde_json::to_string(&cloned).unwrap();

    Response::builder()
        .header("ETag", etag)
        .header("Content-Type", "application/json")
        .body(Body::from(json))
        .unwrap()
}
