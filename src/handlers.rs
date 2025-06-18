use std::ops::Add;
use axum::{extract::{Path, State}, Json};
use uuid::Uuid;
use rand::RngCore;
use aes_gcm::aead::{Aead};
use aes_gcm::aead::generic_array::GenericArray;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{Utc, Duration, DateTime};
use serde::{Deserialize, Serialize};
use tracing::error;
use validator::{Validate, ValidationError};
use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Deserialize, Validate)]
pub struct CreateSecretRequest {
    #[validate(length(min = 1, max = 10000))]
    value: String,
}

#[derive(Serialize)]
pub struct CreateSecretResponse {
    link: String,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SecretResponse {
    value: String,
}

pub async fn create_secret(
    State(state): State<AppState>,
    Json(secret): Json<CreateSecretRequest>,
) -> Result<Response, AppError> {

    //secret.validate().map_err()
    //secret.validate().map_err(|e| {})

    let id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::hours(1);

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let nonce = GenericArray::from_slice(&nonce_bytes);
    let ciphertext = state.cipher
        .encrypt(nonce, secret.value.as_bytes())
        .map_err(|_| AppError::InternalServerError)?;

    sqlx::query!(
        "INSERT INTO secrets (id, secret, nonce, expires_at) VALUES ($1, $2, $3, $4)",
        id,
        &ciphertext,
        &nonce_bytes,
        expires_at,
    )
        .execute(&state.db)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let location = format!("/secret/{}", id);

    let body = CreateSecretResponse {
        expires_at: Utc::now().add(Duration::hours(1)),
        link: location.clone(),
    };

    let mut headers = HeaderMap::new();
    headers.insert("Location", HeaderValue::from_str(&location).unwrap());

    let resp = (StatusCode::CREATED, headers, Json(body)).into_response();
    Ok(resp)
}

pub async fn retrieve_secret(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {

    let uuid = Uuid::parse_str(id.as_str())
        .map_err(move |_| AppError::InternalServerError)?;
    let record = sqlx::query!(
        "SELECT secret, nonce, expires_at, claimed FROM secrets WHERE id = $1",
        uuid
    )
        .fetch_optional(&state.db)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let record = record.ok_or(AppError::NotFound)?;

    // If the secret has already been claimed or expired, attempt to delete it.
    // If the delete fails, it's not an issue as there is a process that periodically
    // runs to clean up expired and claimed secrets.
    if record.claimed || Utc::now() > record.expires_at {
        let del_res = sqlx::query!(
            "DELETE FROM secrets WHERE id = $1", uuid
        )
        .execute(&state.db)
        .await;
        if let Err(_) = del_res {
            error!("Failed to delete expired secret: {}", uuid);
        }
        return Err(AppError::Expired);
    }

    let nonce = GenericArray::from_slice(&record.nonce);
    let secret = state.cipher
        .decrypt(nonce, record.secret.as_slice())
        .map_err(|_| AppError::InternalServerError)?;

    // If marking the secret as claimed doesn't work, return an Http 500 error so
    // that the client can attempt to fetch the secret again while the app maintains
    // properly enforcing the secret can only be fetched once.
    sqlx::query!("UPDATE secrets SET claimed = true WHERE id = $1", uuid)
        .execute(&state.db)
        .await
        .map_err(|_| AppError::InternalServerError)?;

    let body = SecretResponse {
        value: String::from_utf8(secret).unwrap(),
    };

    let resp = (StatusCode::OK, Json(body)).into_response();
    Ok(resp)
}