//! Axum router construction.
//!
//! Routes are registered dynamically based on what is in the database.
//! Collections get full CRUD; singletons get GET/PUT/PATCH.
//! A catch-all layer dispatches unknown resources through the same handlers
//! because the original jsond supports creating resources that don't exist yet (POST).

use axum::http::{HeaderValue, Method, header};
use axum::{Router, middleware as axum_middleware};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::cli::CliArgs;
use crate::db::Database;
use crate::middleware::delay::DelayLayer;
use crate::middleware::read_only::read_only_guard;
use crate::routes;

pub fn build_router(db: &Database, args: &CliArgs) -> Router {
    let public_dir = args.r#static.as_path();
    let serve_dir = if public_dir.exists() && public_dir.is_dir() {
        Some(ServeDir::new(public_dir))
    } else {
        None
    };

    // let cors = CorsLayer::new()
    //     .allow_origin(Any)
    //     .allow_methods([
    //         Method::GET,
    //         Method::HEAD,
    //         Method::POST,
    //         Method::PUT,
    //         Method::PATCH,
    //         Method::DELETE,
    //     ])
    //     .allow_headers(Any)
    //     .allow_credentials(false);
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_origin(Any);

    // We need a small shim so that GET /:resource routes to the right handler
    // depending on whether the resource is a collection or singleton.
    let api = Router::new()
        .merge(routes::root::router())
        // .route("/{resource}", get(resource_dispatcher))
        .merge(routes::singleton::router())
        .merge(routes::collection::router())
        .with_state(db.clone());

    let api = if args.readonly {
        api.layer(axum_middleware::from_fn(read_only_guard))
    } else {
        api
    };
    let api = if args.cors { api.layer(cors) } else { api };

    let api = api
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        ));

    let api = if args.delay > 0 {
        api.layer(DelayLayer::new(args.delay))
    } else {
        api
    };

    // ── Static files (fallback) ───────────────────────────────────────────────
    if let Some(serve) = serve_dir {
        api.fallback_service(serve)
    } else {
        api
    }
}
