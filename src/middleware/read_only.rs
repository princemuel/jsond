//! Middleware that rejects write methods when `--readonly` is set.

use axum::Json;
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use serde_json::{Value, json};

pub async fn read_only_guard(req: Request, next: Next) -> Result<Response, (StatusCode, Json<Value>)> {
    if matches!(
        req.method().to_owned(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        return Ok(next.run(req).await);
    }
    Err((
        StatusCode::FORBIDDEN,
        Json(json!({ "error": "Server is running in readonly mode" })),
    ))
}
