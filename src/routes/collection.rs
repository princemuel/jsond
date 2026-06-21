//! Handlers for collection resources (top-level arrays in the JSON db).
//!
//! Routes:
//!   GET    /:resource            - list all (with filter/sort/paginate/embed)
//!   POST   /:resource            - create one
//!   GET    /{resource}/{id}      - get one
//!   PUT    /{resource}/{id}      - full replace
//!   PATCH  /{resource}/{id}      - partial update
//!   DELETE /{resource}/{id}      - delete

use axum::Router;
use axum::routing::get;

use crate::db::Database;

pub fn router() -> Router<Database> {
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
    use crate::error::Error;
    use crate::query::{self, Pagination};

    /// GET /{resource}
    pub(super) async fn get_all(
        Path(resource): Path<String>,
        Query(params): Query<HashMap<String, String>>,
        State(db): State<Database>,
    ) -> Result<impl IntoResponse, Error> {
        if db.is_singleton(&resource).await {
            let val = db.get_singleton(&resource).await.ok_or(Error::NotFound)?;
            return Ok(Json(val).into_response());
        }

        let raw_items = db.get_collection(&resource).await.ok_or(Error::NotFound)?;

        let qp = query::parse(&params);
        let res = query::apply(raw_items, &qp);
        let total = res.total;

        let mut items = res.items;
        for embed in &qp.embed {
            helpers::attach_has_many(&db, &resource, embed, &mut items).await;
        }
        for expand in &qp.expand {
            helpers::attach_belongs_to(&db, expand, &mut items).await;
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Total-Count",
            total
                .to_string()
                .parse()
                .map_err(|_e| Error::InvalidHeader)?,
        );

        let body = match res.pagination {
            Pagination::Page { page, per_page } => {
                let pages = total.div_ceil(per_page).max(1);
                json!({
                    "first": 1,
                    "prev":  (page > 1).then(|| page - 1),
                    "next":  (page < pages).then(|| page + 1),
                    "last":  pages,
                    "pages": pages,
                    "items": total,
                    "data":  items,
                })
            }
            _ => json!(items),
        };

        Ok((StatusCode::OK, headers, Json(body)).into_response())
    }

    /// GET /{resource}/id
    pub(super) async fn get(
        Path((resource, id)): Path<(String, String)>,
        Query(params): Query<HashMap<String, String>>,
        State(db): State<Database>,
    ) -> Result<impl IntoResponse, Error> {
        let mut item = db.find(&resource, &id).await.ok_or(Error::NotFound)?;

        // Support _embed / _expand on single-item GETs
        let embed_keys: Vec<_> = params
            .get("_embed")
            .map(|s| s.split(',').map(str::trim).collect())
            .unwrap_or_default();

        let expand_keys: Vec<_> = params
            .get("_expand")
            .map(|s| s.split(',').map(str::trim).collect())
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
    ) -> Result<impl IntoResponse, Error> {
        let item = db.insert(&resource, body).await?;
        Ok((StatusCode::CREATED, Json(item)))
    }

    /// PUT /{resource}/id
    pub(super) async fn put(
        Path((resource, id)): Path<(String, String)>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse, Error> {
        let item = db.replace(&resource, &id, body).await?;
        Ok(Json(item))
    }

    /// PATCH /{resource}/id
    pub(super) async fn patch(
        Path((resource, id)): Path<(String, String)>,
        State(db): State<Database>,
        Json(body): Json<Value>,
    ) -> Result<impl IntoResponse, Error> {
        let item = db.patch(&resource, &id, body).await?;
        Ok(Json(item))
    }

    /// DELETE /{resource}/id?_dependent=<collection>
    pub(super) async fn delete(
        Path((resource, id)): Path<(String, String)>,
        State(db): State<Database>,
        Query(params): Query<HashMap<String, String>>,
    ) -> Result<impl IntoResponse, Error> {
        let dependent = params.get("_dependent").map(String::as_str);
        let _deleted = db.delete(&resource, &id, dependent).await?;
        Ok(StatusCode::NO_CONTENT)
    }
}

mod helpers {
    use std::collections::HashMap;

    use serde_json::Value;

    use crate::db::{Database, as_str_or_number_string, singular};

    /// `_embed=comments` -> hasMany.
    /// For each item, attaches `comments: [...]` where `comment.postId ==
    /// item.id`.
    ///
    /// The foreign-key name is derived as: `singular(parent_resource)` + "Id".
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

        // Group children by their fk value once, instead of re-scanning the
        // whole `children` vec for every item.
        let mut groups: HashMap<String, Vec<&Value>> = HashMap::new();

        for child in &children {
            if let Some(key) = child.get(&fk).and_then(as_str_or_number_string) {
                groups.entry(key).or_default().push(child);
            }
        }

        for item in items.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };
            let Some(parent_id) = obj.get("id").and_then(as_str_or_number_string) else {
                continue;
            };

            let related = groups
                .get(&parent_id)
                .map(|v| v.iter().map(|c| (*c).to_owned()).collect())
                .unwrap_or_default();

            obj.insert(embed.to_owned(), Value::Array(related));
        }
    }

    /// `_expand=post` -> belongsTo.
    /// For each item, attaches `post: {...}` by looking up `item.postId` in the
    /// parent collection. We try `{expand}s` first (e.g. "posts"), then the
    /// name as-is (e.g. "people"), matching json-server's own pluralisation
    /// logic.
    pub(super) async fn attach_belongs_to(db: &Database, expand: &str, items: &mut [Value]) {
        // Try plural first, then bare name (handles irregular plurals like "people")
        let plural = format!("{expand}s");

        let Some(parents) = (match db.get_collection(&plural).await {
            Some(col) => Some(col),
            None => db.get_collection(expand).await,
        }) else {
            return;
        };

        // Group parents by id once; belongs_to only needs one match per key.
        let mut by_id = HashMap::new();

        for parent in &parents {
            if let Some(key) = parent.get("id").and_then(as_str_or_number_string) {
                by_id.entry(key).or_insert(parent);
            }
        }

        let fk = format!("{expand}Id"); // e.g. "postId"

        for item in items.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };

            let Some(fk) = obj.get(&fk).and_then(as_str_or_number_string) else {
                continue;
            };

            let parent = by_id
                .get(&fk)
                .map_or_else(|| Value::Null, |v| (*v).to_owned());
            obj.insert(expand.to_owned(), parent);
        }
    }
}
