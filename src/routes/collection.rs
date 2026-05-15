//! Handlers for collection resources (top-level arrays in the JSON db).
//!
//! Routes:
//!   GET    /:resource          — list (with filter/sort/paginate/embed)
//!   GET    /{resource}/{id}      — get one
//!   POST   /:resource          — create
//!   PUT    /{resource}/{id}      — full replace
//!   PATCH  /{resource}/{id}      — partial update
//!   DELETE /{resource}/{id}      — delete

use axum::Router;
use axum::routing::get;

use crate::db::Database;

pub(crate) fn router() -> Router<Database> {
    Router::new()
        .route("/{resource}", get(handlers::get_all).post(handlers::post))
        .route(
            "/{resource}/{id}",
            get(handlers::get)
                .put(handlers::put)
                .patch(handlers::patch)
                .delete(handlers::delete),
        )
}

mod handlers {
    use std::collections::HashMap;

    use axum::Json;
    use axum::extract::{Path, Query, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use serde_json::{Value, json};

    use super::helpers;
    use crate::db::Database;
    use crate::error::{Error, Result};
    use crate::query::{self, Pagination};

    pub(super) async fn get_all(
        Path(resource): Path<String>,
        Query(params): Query<HashMap<String, String>>,
        State(db): State<Database>,
    ) -> Result<impl IntoResponse> {
        // Singletons bypass all query logic
        if db.is_singleton(&resource).await {
            let val = db.get_singleton(&resource).await.ok_or(Error::NotFound)?;
            return Ok(Json(val).into_response());
        }

        let raw_items = db.get_collection(&resource).await.ok_or(Error::NotFound)?;

        // Parse query params (embed/expand keys are inside qp too)
        let qp = query::parse(&params);
        let res = query::apply(raw_items, &qp);

        // Total is from BEFORE pagination but AFTER filter/search
        let total = match res.pagination {
            Pagination::Page { total, .. } | Pagination::Slice { total, .. } => total,
            Pagination::None => res.items.len(),
        };

        // Attach _embed (hasMany) and _expand (belongsTo) AFTER paging —
        // we embed only the visible page, not the whole collection.
        let mut items = res.items;
        for embed in &qp.embed {
            helpers::attach_has_many(&db, &resource, embed, &mut items).await;
        }
        for expand in &qp.expand {
            helpers::attach_belongs_to(&db, expand, &mut items).await;
        }

        let mut headers = HeaderMap::new();
        // TODO: remove this unwrap once we have a proper header abstraction in place.
        #[expect(clippy::unwrap_used)]
        headers.insert("X-Total-Count", total.to_string().parse().unwrap());

        let body = match res.pagination {
            Pagination::Page {
                page,
                per_page,
                total,
            } => {
                let pages = total.div_ceil(per_page).max(1);
                json!({
                    "first": 1,
                    "prev":  (page > 1).then(|| page - 1),
                    "next":  (page < pages).then(|| page + 1),
                    "last":  pages,
                    "pages": pages,
                    "items": total,   // spec: "items" = total count
                    "data":  items,   // spec: "data"  = page records
                })
            }
            // Slice or no pagination → plain array
            _ => json!(items),
        };

        Ok((StatusCode::OK, headers, Json(body)).into_response())
    }

    pub(super) async fn get(
        Path((resource, id)): Path<(String, String)>,
        Query(params): Query<HashMap<String, String>>,
        State(db): State<Database>,
    ) -> Result<impl IntoResponse> {
        let mut item = db.find(&resource, &id).await.ok_or(Error::NotFound)?;

        // Support _embed / _expand on single-item GETs
        let embed_keys: Vec<String> = params
            .get("_embed")
            .map(|s| s.split(',').map(str::trim).map(String::from).collect())
            .unwrap_or_default();

        let expand_keys: Vec<String> = params
            .get("_expand")
            .map(|s| s.split(',').map(str::trim).map(String::from).collect())
            .unwrap_or_default();

        let mut items = vec![item];
        for embed in &embed_keys {
            helpers::attach_has_many(&db, &resource, embed, &mut items).await;
        }
        for expand in &expand_keys {
            helpers::attach_belongs_to(&db, expand, &mut items).await;
        }
        item = items.remove(0);

        Ok(Json(item))
    }

    pub(super) async fn post(
        Path(resource): Path<String>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse> {
        let item = db.insert(&resource, body).await?;
        Ok((StatusCode::CREATED, Json(item)))
    }

    pub(super) async fn put(
        Path((resource, id)): Path<(String, String)>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse> {
        let item = db.replace(&resource, &id, body).await?;
        Ok(Json(item))
    }

    pub(super) async fn patch(
        Path((resource, id)): Path<(String, String)>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse> {
        let item = db.patch(&resource, &id, body).await?;
        Ok(Json(item))
    }

    /// DELETE /{resource}/id?_dependent=<collection>
    pub(super) async fn delete(
        Path((resource, id)): Path<(String, String)>,
        State(db): State<Database>,
        Query(params): Query<HashMap<String, String>>,
    ) -> Result<impl IntoResponse> {
        let dependent = params.get("_dependent").map(String::as_str);
        db.delete(&resource, &id, dependent).await?;
        Ok(StatusCode::NO_CONTENT)
    }
}

mod helpers {
    use serde_json::Value;

    use crate::db::Database;

    /// `_embed=comments` — hasMany.
    /// For each item, attaches `comments: [...]` where `comment.postId ==
    /// item.id`.
    ///
    /// The foreign-key name is derived: `singular(parent_resource)` + "Id".
    pub(super) async fn attach_has_many(
        db: &Database,
        resource: &str, // parent, e.g. "posts"
        embed: &str,    // child collection, e.g. "comments"
        items: &mut [Value],
    ) {
        let Some(children) = db.get_collection(embed).await else {
            return;
        };

        let fk = format!("{}Id", singular(resource)); // e.g. "postId"

        for item in items.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };

            #[expect(clippy::pattern_type_mismatch)]
            let parent_id = match obj.get("id") {
                Some(Value::String(v)) => v.to_owned(),
                Some(v) => v.to_string(),
                None => continue,
            };

            let related = children
                .iter()
                .filter(|child| {
                    #[expect(clippy::pattern_type_mismatch)]
                    child.get(&fk).is_some_and(|v| match v {
                        Value::String(v) => v == &parent_id,
                        v => v.to_string().trim_matches('"') == parent_id,
                    })
                })
                .cloned()
                .collect();

            obj.insert(embed.to_owned(), Value::Array(related));
        }
    }

    /// `_expand=post` — belongsTo.
    /// For each item, attaches `post: {...}` by looking up `item.postId` in the
    /// parent collection. We try `{expand}s` first (e.g. "posts"), then the
    /// name as-is (e.g. "people"), matching json-server's own pluralisation
    /// logic.
    pub(super) async fn attach_belongs_to(
        db: &Database,
        expand: &str, // singular name of parent, e.g. "post"
        items: &mut [Value],
    ) {
        // Try plural first, then bare name (handles irregular plurals like "people")
        let plural = format!("{expand}s");
        let parents = match db.get_collection(&plural).await {
            Some(col) => col,
            None => match db.get_collection(expand).await {
                Some(col) => col,
                None => return,
            },
        };

        let fk = format!("{expand}Id"); // e.g. "postId"

        for item in items.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };

            let Some(fk_val) = obj.get(&fk) else { continue };
            #[expect(clippy::pattern_type_mismatch)]
            let fk_str = match fk_val {
                Value::String(v) => v.to_owned(),
                v => v.to_string(),
            };
            let parent = parents
                .iter()
                .find(|parent| {
                    #[expect(clippy::pattern_type_mismatch)]
                    parent.get("id").is_some_and(|v| match v {
                        Value::String(v) => v == &fk_str,
                        v => v.to_string().trim_matches('"') == fk_str,
                    })
                })
                .cloned()
                .unwrap_or(Value::Null);

            obj.insert(expand.to_owned(), parent);
        }
    }

    /// "posts" → "post", "comments" → "comment", "people" → "people" (no
    /// trailing s)
    fn singular(s: &str) -> &str { s.strip_suffix('s').unwrap_or(s) }
}
