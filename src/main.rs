mod error;
mod app_state;
mod crypto;
mod handlers;

use axum::{Router};
use axum::routing::{get, post};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use crate::app_state::AppState;
use crate::handlers::{create_secret, retrieve_secret};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    
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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await?;

    axum::serve(listener, app).await?;
    
    Ok(())
}