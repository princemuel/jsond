use axum::Router;
use axum::routing::get;

use crate::db::Database;

pub(crate) fn router() -> Router<Database> {
    Router::new().route("/", get(handlers::get))
}

mod handlers {
    use axum::Json;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use serde_json::json;

    use crate::db::Database;

    /// GET / — return all resource names and example routes.
    pub(super) async fn get(State(db): State<Database>) -> impl IntoResponse {
        Json(json!({ "resources": db.resources().await }))
    }
}
