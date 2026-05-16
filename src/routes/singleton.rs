//! Handlers for singleton resources (top-level objects in the JSON db).
//!
//! Routes:
//!   GET    /:resource   — get singleton
//!   PUT    /:resource   — full replace
//!   PATCH  /:resource   — partial update

use axum::Router;
use axum::routing::put;

use crate::db::Database;

pub fn router() -> Router<Database> {
    Router::new().route("/{resource}", put(handlers::put).patch(handlers::patch))
}

mod handlers {

    use axum::Json;
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    use serde_json::Value;

    use crate::db::Database;
    use crate::error::{Error, Result};

    pub(super) async fn put(
        Path(resource): Path<String>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse> {
        if !db.is_singleton(&resource).await {
            return Err(Error::NotFound);
        }

        let item = db.replace_singleton(&resource, body).await?;
        Ok(Json(item))
    }

    pub(super) async fn patch(
        Path(resource): Path<String>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse> {
        if !db.is_singleton(&resource).await {
            return Err(Error::NotFound);
        }

        let item = db.patch_singleton(&resource, body).await?;
        Ok(Json(item))
    }
}
