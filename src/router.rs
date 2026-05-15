use axum::{Router, middleware as axum_middleware};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::cli::CliArgs;
use crate::db::Database;
use crate::middleware::delay::DelayLayer;
use crate::middleware::read_only::read_only_guard;
use crate::routes;

pub(crate) fn build_router(db: &Database, args: &CliArgs) -> Router {
    let router = Router::new()
        .merge(routes::root::router())
        .merge(routes::singleton::router())
        .merge(routes::collection::router())
        .with_state(db.clone());

    let mut api = router.layer(TraceLayer::new_for_http());
    if args.readonly {
        api = api.layer(axum_middleware::from_fn(read_only_guard));
    }

    if args.cors {
        // TODO: setting the layer as permissive for now. to be updated
        api = api.layer(CorsLayer::permissive());
    }

    if args.delay > 0 {
        api = api.layer(DelayLayer::new(args.delay));
    }

    if args.r#static.is_dir() {
        api = api.fallback_service(ServeDir::new(args.r#static.as_path()));
    }

    api
}
