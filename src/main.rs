mod error;
mod app_state;
mod crypto;
mod handlers;
mod metrics;

use crate::app_state::AppState;
use crate::handlers::{create_secret, retrieve_secret};
use axum::routing::{get, post};
use axum::{middleware, Router};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use tokio::join;
use tracing::{info, Level};

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
        .route_layer(middleware::from_fn(metrics::record_metrics))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await?;

    let metric_app = metrics::metrics_app();
    let prom_listener = tokio::net::TcpListener::bind("127.0.0.1:8082")
        .await?;

    info!("Starting http server on {}", listener.local_addr()?);

    // Run the main application server and prometheus server concurrently
    let (main_server, prom_server) = join!(
        axum::serve(listener, app),
        axum::serve(prom_listener, metric_app)
    );

    // Handle errors from either server
    main_server?;
    prom_server?;
    
    Ok(())
}