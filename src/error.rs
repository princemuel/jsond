use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("method not allowed")]
    MethodNotAllowed,

    #[error("this resource already exists")]
    Conflict,

    #[error("'{0}' is not a collection resource")]
    NotCollection(String),

    #[error("unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Readonly mode: write operations are disabled")]
    ReadOnly,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, self.to_string()),
            Self::Conflict => (StatusCode::CONFLICT, self.to_string()),
            Self::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            Self::ReadOnly => (StatusCode::FORBIDDEN, self.to_string()),
            Self::Json(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            Self::Io(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown Error".to_owned()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}
