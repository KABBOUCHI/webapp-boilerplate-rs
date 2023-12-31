use anyhow::Result;
use axum::{
    extract::{Path, Query},
    http::Request,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use axum_trace_id::{SetTraceIdLayer, TraceId};
use tower_http::trace::TraceLayer;

use dotenv::dotenv;
use models::*;
use queue::Queue;
use std::{env, net::SocketAddr};
use tracing::{info, info_span};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt().json().init();

    ensemble::setup(&env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
        .expect("Failed to set up database pool.");

    let app = Router::new()
        .route("/", get(hello_world))
        .route("/users", get(list_users))
        .route("/users/:user_id/posts", get(list_user_posts))
        .route("/posts", get(list_posts))
        .route("/job", get(job))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                if let Some(trace) = request.extensions().get::<TraceId<String>>() {
                    info_span!("http_request", trace_id = trace.id)
                } else {
                    info_span!("http_request", trace_id = "none")
                }
            }),
        )
        .layer(SetTraceIdLayer::<String>::new().with_header_name("X-Request-Id"));

    let address = SocketAddr::from((
        [0, 0, 0, 0],
        env::var("PORT").map_or(8000, |p| p.parse().unwrap()),
    ));
    info!("⚡ PingCRM started on http://{address}");

    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn hello_world() -> impl IntoResponse {
    "Hello, World!"
}

#[derive(serde::Deserialize)]
struct ListUsersQueryParams {
    #[serde(default)]
    include_posts: bool,
}

async fn list_users(params: Query<ListUsersQueryParams>) -> Json<Vec<User>> {
    let users = User::query()
        .when(params.include_posts, |q| q.with("posts"))
        .get()
        .await
        .unwrap();

    Json(users)
}

async fn list_user_posts(Path(user_id): Path<u64>) -> Json<Vec<Post>> {
    let posts = Post::query()
        .r#where("user_id", "=", user_id)
        .get()
        .await
        .unwrap();

    Json(posts)
}

async fn list_posts() -> Json<Vec<Post>> {
    let posts = Post::all().await.unwrap();

    Json(posts)
}
async fn job() -> String {
    Queue::dispatch(&queue::jobs::MyJob::new(1)).await.unwrap();

    "Queued".to_string()
}
