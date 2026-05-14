//! Handlers for singleton resources (top-level objects in the JSON db).
//!
//! Routes:
//!   GET    /:resource   — get singleton
//!   PUT    /:resource   — full replace
//!   PATCH  /:resource   — partial update

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::Value;
use tracing::debug;

use crate::db::Db;
use crate::error::{Error, Result};

mod handlers {}

pub async fn get_singleton(
    State(db): State<Db>,
    Path(resource): Path<String>,
) -> Result<Json<Value>> {
    let guard = db.read().await;

    if !guard.is_singleton(&resource) {
        return Err(Error::NotFound);
    }

    let item = guard.get_singleton(&resource).ok_or(Error::NotFound)?;
    Ok(Json(item))
}

pub async fn replace_singleton(
    State(db): State<Db>,
    Path(resource): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let mut guard = db.write().await;

    if guard.read_only {
        return Err(Error::ReadOnly);
    }

    if !guard.is_singleton(&resource) {
        return Err(Error::NotFound);
    }

    if !body.is_object() {
        return Err(Error::UnprocessableEntity("Request body must be a JSON object".into()));
    }

    let updated = guard.replace_singleton(&resource, body)?;
    guard.persist()?;

    debug!(resource = %resource, "singleton replaced");
    Ok(Json(updated))
}

pub async fn patch_singleton(
    State(db): State<Db>,
    Path(resource): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let mut guard = db.write().await;

    if guard.read_only {
        return Err(Error::ReadOnly);
    }

    if !guard.is_singleton(&resource) {
        return Err(Error::NotFound);
    }

    if !body.is_object() {
        return Err(Error::UnprocessableEntity("Request body must be a JSON object".into()));
    }

    let updated = guard.patch_singleton(&resource, body)?;
    guard.persist()?;

    debug!(resource = %resource, "singleton patched");
    Ok(Json(updated))
}

/// Fallback: DELETE on a singleton returns 405.
pub async fn delete_singleton() -> (StatusCode, Json<Value>) {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(serde_json::json!({ "error": "Cannot DELETE a singleton resource" })),
    )
}
