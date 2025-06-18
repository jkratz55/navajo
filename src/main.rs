mod error;
mod app_state;
mod crypto;
mod handlers;

use std::future::ready;
use std::time::Instant;
use axum::{middleware, Router};
use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use tracing::{info, Level};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use tokio::join;
use crate::app_state::AppState;
use crate::handlers::{create_secret, retrieve_secret};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .with_current_span(true)
        .with_span_list(true)
        .with_level(true)
        .with_thread_names(true)
        .with_max_level(Level::INFO)
        .init();


    info!("Initializing application");

    let database_url = std::env::var("DATABASE_URL")?;
    let encryption_key = std::env::var("AES_ENCRYPTION_KEY")?;

    let cipher = crypto::build_cipher(&encryption_key)?;
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    
    let state = AppState::new(db, cipher);
    
    let app = Router::new()
        .route("/secret", post(create_secret))
        .route("/secret/{id}", get(retrieve_secret))
        .route_layer(middleware::from_fn(record_metrics))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await?;
    
    let metrics = metrics_app();
    let prom_listener = tokio::net::TcpListener::bind("127.0.0.1:8082")
        .await?;

    info!("Starting http server on {}", listener.local_addr()?);

    // Run the main application server and prometheus server concurrently
    let (mainServer, promServer) = join!(
        axum::serve(listener, app),
        axum::serve(prom_listener, metrics)
    );

    // Handle errors from either server
    mainServer?;
    promServer?;
    
    Ok(())
}

fn metrics_app() -> Router {
    const EXPONENTIAL_SECONDS: &[f64] = &[0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000];
    
    let prom_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS
        )
        .unwrap()
        .install_recorder()
        .unwrap();
    
    Router::new().route("/metrics", get(move || ready(prom_handle.render())))
}

async fn record_metrics(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();
    
    let response = next.run(req).await;
    
    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);
    
    response
}