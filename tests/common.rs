//! Shared test harness.
//!
//! `TestServer` spins up a real axum server on a random OS-assigned port per
//! test, backed by a `NamedTempFile`. Everything is real — real TCP, real
//! JSON parsing, real file I/O.  No mocking.
//!
//! The server task is detached with `tokio::spawn`; it is implicitly cancelled
//! when the test runtime shuts down after each `#[tokio::test]`.
#![expect(clippy::unwrap_used)]

use axum::Router;
use axum::http::{Method, header};
use jsond::db::Database;
use jsond::id::IdStrategy;
use jsond::routes;
use serde_json::{Value, json};
use tempfile::NamedTempFile;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

pub struct TestServer {
    pub base_url: String,
    pub client: reqwest::Client,
    // stays alive for the test duration. dropping it deletes the file.
    _db_file: NamedTempFile,
}

impl TestServer {
    /// Spin up with `UUIDv7` ids (sensible default for most tests).
    pub async fn new(db_content: Value) -> Self {
        Self::with_strategy(db_content, IdStrategy::Uuidv7).await
    }

    /// Spin up with an explicit `IdStrategy` (used by id-strategy tests).
    pub async fn with_strategy(db_content: Value, strategy: IdStrategy) -> Self {
        let mut file = NamedTempFile::new().unwrap();
        serde_json::to_writer_pretty(&mut file, &db_content).unwrap();

        let db = Database::load(file.path(), strategy, false).unwrap();
        let app = build_router(db);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { base_url: format!("http://{addr}"), client: reqwest::Client::new(), _db_file: file }
    }

    fn url(&self, path: &str) -> String { format!("{}{}", self.base_url, path) }

    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.client.get(self.url(path)).send().await.unwrap()
    }

    pub async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.client.post(self.url(path)).json(&body).send().await.unwrap()
    }

    pub async fn put(&self, path: &str, body: Value) -> reqwest::Response {
        self.client.put(self.url(path)).json(&body).send().await.unwrap()
    }

    pub async fn patch(&self, path: &str, body: Value) -> reqwest::Response {
        self.client.patch(self.url(path)).json(&body).send().await.unwrap()
    }

    pub async fn delete(&self, path: &str) -> reqwest::Response {
        self.client.delete(self.url(path)).send().await.unwrap()
    }

    /// DELETE with arbitrary query string, e.g. `"_dependent=comments"`.
    pub async fn delete_qs(&self, path: &str, qs: &str) -> reqwest::Response {
        self.client.delete(format!("{}{}?{}", self.base_url, path, qs)).send().await.unwrap()
    }

    /// GET with arbitrary query string, e.g. `"author=alice&views:gt=50"`.
    pub async fn get_qs(&self, path: &str, qs: &str) -> reqwest::Response {
        self.client.get(format!("{}{}?{}", self.base_url, path, qs)).send().await.unwrap()
    }

    // ── Convenience: deserialise to Value directly ────────────────────────────

    pub async fn get_json(&self, path: &str) -> Value { self.get(path).await.json().await.unwrap() }

    pub async fn get_qs_json(&self, path: &str, qs: &str) -> Value {
        self.get_qs(path, qs).await.json().await.unwrap()
    }

    pub async fn post_json(&self, path: &str, body: Value) -> Value {
        self.post(path, body).await.json().await.unwrap()
    }

    pub async fn patch_json(&self, path: &str, body: Value) -> Value {
        self.patch(path, body).await.json().await.unwrap()
    }

    pub async fn put_json(&self, path: &str, body: Value) -> Value {
        self.put(path, body).await.json().await.unwrap()
    }
}

// mirrors production build_router without the CLI-only bits

fn build_router(db: Database) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT])
        .allow_origin(Any);

    Router::new()
        .merge(routes::root::router())
        .merge(routes::singleton::router())
        .merge(routes::collection::router())
        .layer(cors)
        .with_state(db)
}

#[must_use]
pub fn fixture_db() -> Value {
    json!({
        "posts": [
            { "id": "1", "title": "Hello World",            "author": "alice", "views": 10  },
            { "id": "2", "title": "Rust is Fast",            "author": "bob",   "views": 250 },
            { "id": "3", "title": "Axum is Ergonomic",      "author": "alice", "views": 80  },
            { "id": "4", "title": "Zero Cost Abstractions",  "author": "carol", "views": 500 }
        ],
        "comments": [
            { "id": "1", "body": "Great post!",   "postId": "1", "rating": 5 },
            { "id": "2", "body": "Very helpful",  "postId": "1", "rating": 4 },
            { "id": "3", "body": "I agree",       "postId": "2", "rating": 3 },
            { "id": "4", "body": "Nice writeup",  "postId": "3", "rating": 5 }
        ],
        "tags": [
            { "id": "1", "label": "rust",   "postId": "1" },
            { "id": "2", "label": "axum",   "postId": "3" },
            { "id": "3", "label": "perf",   "postId": "2" }
        ],
        "profile": {
            "name": "admin",
            "email": "admin@example.com",
            "role": "superuser"
        }
    })
}

/// Minimal db for cascade-delete tests (isolated so counts are predictable).
#[must_use]
pub fn cascade_db() -> Value {
    json!({
        "posts":    [{ "id": "1" }, { "id": "2" }],
        "comments": [
            { "id": "1", "postId": "1" },
            { "id": "2", "postId": "1" },
            { "id": "3", "postId": "2" }
        ]
    })
}

/// Extract all `id` strings from a JSON array.
/// E.g. `ids(&db["posts"])` returns `vec!["1", "2", "3", "4"]`.
#[must_use]
pub fn ids(arr: &Value) -> Vec<&str> {
    arr.as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap()).collect()
}

/// Extract a numeric field from every item in a JSON array.
#[must_use]
pub fn nums(arr: &Value, field: &str) -> Vec<i64> {
    arr.as_array().unwrap().iter().map(|v| v[field].as_i64().unwrap()).collect()
}

/// Assert a JSON array is sorted ascending by a numeric field.
pub fn assert_sorted_asc(arr: &Value, field: &str) {
    let v = nums(arr, field);
    let mut sorted = v.clone();
    sorted.sort_unstable();
    assert_eq!(v, sorted, "expected {field} ascending, got {v:?}");
}

/// Assert a JSON array is sorted descending by a numeric field.
pub fn assert_sorted_desc(arr: &Value, field: &str) {
    let v = nums(arr, field);
    let mut sorted = v.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(v, sorted, "expected {field} descending, got {v:?}");
}
