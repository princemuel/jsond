//! Handlers for collection resources (top-level arrays in the JSON db).
//!
//! Routes:
//!   GET    /:resource          — list (with filter/sort/paginate/embed)
//!   GET    /{resource}/{id}      — get one
//!   POST   /:resource          — create
//!   PUT    /{resource}/{id}      — full replace
//!   PATCH  /{resource}/{id}      — partial update
//!   DELETE /{resource}/{id}      — delete

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::db::Db;
use crate::error::{Error, Result};
use crate::query::{Pagination, apply_query, embed_children, expand_parents, parse_query};

pub(crate) fn router() -> Router<Db> { Router::new() }

mod handlers {}

// ──────────────────────────────────────────────────────────────────────────────
// GET /:resource
// ──────────────────────────────────────────────────────────────────────────────

pub async fn list_collection(
    State(db): State<Db>,
    Path(resource): Path<String>,
    Query(raw_params): Query<HashMap<String, String>>,
) -> Result<(StatusCode, HeaderMap, Json<Value>)> {
    let guard = db.read().await;

    if !guard.is_collection(&resource) {
        return Err(Error::NotFound);
    }

    let items = guard.get_collection(&resource).unwrap_or_default();
    let default_per_page = 10_usize; // overridden via CLI args in real flow; kept simple here

    let params = parse_query(&raw_params, default_per_page);
    let result = apply_query(items, &params);

    // Embed / expand after pagination so we don't embed hundreds of records
    let mut data = result.data;
    if let Some(obj) = guard.data.as_object() {
        if !params.embed.is_empty() {
            embed_children(&mut data, obj, params.embed.into_iter());
        }

        if !params.expand.is_empty() {
            expand_parents(&mut data, obj, params.expand.into_iter());
        }
    }

    let mut headers = HeaderMap::new();
    let header =
        HeaderValue::from_str(&result.total.to_string()).map_err(|e| Error::Internal(e.into()))?;
    headers.insert("X-Total-Count", header);

    // Link header for slice pagination
    match &params.pagination {
        Pagination::Slice { .. } => {
            // no link header needed for slice-based
        }
        _ => {}
    }

    let body = match &params.pagination {
        Pagination::Page { .. } => {
            // json-server v1 page response shape
            json!({
                "first": result.first,
                "prev": result.prev,
                "next": result.next,
                "last": result.last,
                "pages": result.pages,
                "items": result.total,
                "page": result.page,
                "per_page": result.per_page,
                "data": data,
            })
        }
        _ => Value::Array(data),
    };

    tracing::debug!(resource = %resource, total = result.total, "list");
    Ok((StatusCode::OK, headers, Json(body)))
}

// ──────────────────────────────────────────────────────────────────────────────
// GET /{resource}/{id}
// ──────────────────────────────────────────────────────────────────────────────

pub async fn get_one(
    State(db): State<Db>,
    Path((resource, id)): Path<(String, String)>,
    Query(raw_params): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    let guard = db.read().await;

    if !guard.is_collection(&resource) {
        return Err(Error::NotFound);
    }

    let mut item = guard.find(&resource, &id).ok_or(Error::NotFound)?;

    // Embed / expand on single resource
    if let Some(embed) = raw_params.get("_embed") {
        let embed = embed.split(',').map(|s| s.trim().to_string());
        if let Some(obj) = guard.data.as_object() {
            let mut items = vec![item];
            embed_children(&mut items, obj, embed);
            item = items.remove(0);
        }
    }

    if let Some(expand) = raw_params.get("_expand") {
        let expand = expand.split(',').map(|s| s.trim().to_owned());
        if let Some(obj) = guard.data.as_object() {
            let mut items = vec![item];
            expand_parents(&mut items, obj, expand);
            item = items.remove(0);
        }
    }

    Ok(Json(item))
}

// ──────────────────────────────────────────────────────────────────────────────
// POST /:resource
// ──────────────────────────────────────────────────────────────────────────────

pub async fn create_item(
    State(db): State<Db>,
    Path(resource): Path<String>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>)> {
    let mut guard = db.write().await;

    if guard.read_only {
        return Err(Error::ReadOnly);
    }

    // Auto-create the collection if it doesn't exist yet
    if !guard.is_collection(&resource) && !guard.is_singleton(&resource) {
        guard.ensure_collection(&resource);
    }

    if !guard.is_collection(&resource) {
        return Err(Error::BadRequest(format!(
            "'{}' is a singleton — use PUT or PATCH to update it",
            resource
        )));
    }

    if !body.is_object() {
        return Err(Error::UnprocessableEntity("Request body must be a JSON object".into()));
    }

    let created = guard.insert(&resource, body)?;
    guard.persist()?;

    tracing::debug!(resource = %resource, id = ?created.get("id"), "created");
    Ok((StatusCode::CREATED, Json(created)))
}

// ──────────────────────────────────────────────────────────────────────────────
// PUT /{resource}/{id}
// ──────────────────────────────────────────────────────────────────────────────

pub async fn replace_item(
    State(db): State<Db>,
    Path((resource, id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let mut guard = db.write().await;

    if guard.read_only {
        return Err(Error::ReadOnly);
    }

    if !guard.is_collection(&resource) {
        return Err(Error::NotFound);
    }

    if !body.is_object() {
        return Err(Error::UnprocessableEntity("Request body must be a JSON object".into()));
    }

    let updated = guard.replace(&resource, &id, body)?;
    guard.persist()?;

    tracing::debug!(resource = %resource, id = %id, "replaced");
    Ok(Json(updated))
}

// ──────────────────────────────────────────────────────────────────────────────
// PATCH /{resource}/{id}
// ──────────────────────────────────────────────────────────────────────────────

pub async fn patch_item(
    State(db): State<Db>,
    Path((resource, id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let mut guard = db.write().await;

    if guard.read_only {
        return Err(Error::ReadOnly);
    }

    if !guard.is_collection(&resource) {
        return Err(Error::NotFound);
    }

    if !body.is_object() {
        return Err(Error::UnprocessableEntity("Request body must be a JSON object".into()));
    }

    let updated = guard.patch(&resource, &id, body)?;
    guard.persist()?;

    tracing::debug!(resource = %resource, id = %id, "patched");
    Ok(Json(updated))
}

// ──────────────────────────────────────────────────────────────────────────────
// DELETE /{resource}/{id}
// ──────────────────────────────────────────────────────────────────────────────

pub async fn delete_item(
    State(db): State<Db>,
    Path((resource, id)): Path<(String, String)>,
) -> Result<Json<Value>> {
    let mut guard = db.write().await;

    if guard.read_only {
        return Err(Error::ReadOnly);
    }

    if !guard.is_collection(&resource) {
        return Err(Error::NotFound);
    }

    let deleted = guard.delete(&resource, &id)?;
    guard.persist()?;

    tracing::debug!(resource = %resource, id = %id, "deleted");
    Ok(Json(deleted))
}
