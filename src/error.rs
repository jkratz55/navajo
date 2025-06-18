use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,
    #[error("Expired")]
    Expired,
    #[error("Claimed")]
    Claimed,
    #[error("Internal Server Error")]
    InternalServerError,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    status: u16,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self { 
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Expired => StatusCode::GONE,
            AppError::Claimed => StatusCode::GONE,
            AppError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        let resp = ErrorResponse{
            status: status.as_u16(),
            message: self.to_string(),
        };
        
        (status, Json(resp)).into_response()
    }
}