//! Axum router construction.
//!
//! Routes are registered dynamically based on what is in the database.
//! Collections get full CRUD; singletons get GET/PUT/PATCH.
//! A catch-all layer dispatches unknown resources through the same handlers —
//! because json-server supports creating resources that don't exist yet (POST).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, middleware as axum_middleware};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::cli::Args;
use crate::db::Db;
use crate::middleware::delay::DelayLayer;
use crate::middleware::read_only::read_only_guard;
use crate::routes;
use crate::routes::collection::{
    create_item,
    delete_item,
    get_one,
    list_collection,
    patch_item,
    replace_item,
};
use crate::routes::singleton::{
    delete_singleton,
    get_singleton,
    patch_singleton,
    replace_singleton,
};

pub fn build_router(db: Db, args: &Args) -> Router {
    // ── Static file service ────────────────────────────────────────────────────
    let static_dir = args.static_dir.clone();
    let serve_dir = if static_dir.exists() { Some(ServeDir::new(&static_dir)) } else { None };

    // We need a small shim so that GET /:resource routes to the right handler
    // depending on whether the resource is a collection or singleton.
    let api = Router::new()
        .merge(routes::root::router())
        .route("/:resource", get(resource_dispatcher).post(create_item))
        .route(
            "/{resource}/{id}",
            get(get_one).put(replace_item).patch(patch_item).delete(delete_item),
        )
        .with_state(Arc::clone(&db));

    let api =
        if args.readonly { api.layer(axum_middleware::from_fn(read_only_guard)) } else { api };
    let api = if !args.no_cors { api.layer(CorsLayer::permissive()) } else { api };
    let api = api.layer(TraceLayer::new_for_http());
    let api = api.layer(SetResponseHeaderLayer::if_not_present(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    ));

    let api = if args.delay > 0 { api.layer(DelayLayer::new(args.delay)) } else { api };

    // ── Static files (fallback) ───────────────────────────────────────────────
    if let Some(serve) = serve_dir { api.fallback_service(serve) } else { api }
}

/// Dispatches GET /:resource to either `list_collection` or `get_singleton`
/// based on the resource type in the database.
async fn resource_dispatcher(
    state: State<Db>,
    path: Path<String>,
    query: Query<HashMap<String, String>>,
) -> Response {
    let db = state.read().await;
    if db.is_singleton(&path.0) {
        drop(db);
        match get_singleton(state, path).await {
            Ok(r) => r.into_response(),
            Err(e) => e.into_response(),
        }
    } else {
        drop(db);
        match list_collection(state, path, query).await {
            Ok(r) => r.into_response(),
            Err(e) => e.into_response(),
        }
    }
}
