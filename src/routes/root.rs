use axum::Router;
use axum::routing::get;

use crate::db::Db;

pub(crate) fn router() -> Router<Db> { Router::new().route("/", get(handlers::get)) }

mod handlers {
    use axum::Json;
    use axum::extract::State;
    use serde_json::{Value, json};

    use crate::db::Db;

    /// GET / — return all resource names and example routes.
    pub(super) async fn get(State(db): State<Db>) -> Json<Value> {
        let db = db.read().await;
        let resources: Vec<Value> = db
            .resource_names()
            .into_iter()
            .map(|name| {
                if db.is_collection(&name) {
                    json!({
                        "resource": name,
                        "type": "collection",
                        "links": {
                            "list": format!("/{}", name),
                            "item": format!("/{}/:id", name),
                        }
                    })
                } else {
                    json!({
                        "resource": name,
                        "type": "singleton",
                        "link": format!("/{}", name),
                    })
                }
            })
            .collect();

        Json(json!({ "resources": resources }))
    }
}
