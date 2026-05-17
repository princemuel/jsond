//! CRUD lifecycle tests: GET, POST, PUT, PATCH, DELETE, cascade delete,
//! id coercion, auto-create collection, error responses.
#![expect(clippy::tests_outside_test_module)]
pub mod common;
use common::{TestServer, cascade_db, fixture_db, ids};
use jsond::id::IdStrategy;
use serde_json::json;

#[tokio::test]
async fn root_lists_all_resources() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_json("/").await;
    let resources = body
        .get("resources")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();

    assert!(resources.contains(&"posts"));
    assert!(resources.contains(&"comments"));
    assert!(resources.contains(&"profile"));
}

//  GET a collection
#[tokio::test]
async fn get_collection_returns_all_items() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_json("/posts").await;
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn get_collection_status_200() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.get("/posts").await.status(), 200);
}

#[tokio::test]
async fn get_collection_has_x_total_count_header() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.get("/posts").await;
    assert_eq!(res.headers().get("x-total-count").unwrap(), "4");
}

#[tokio::test]
async fn get_unknown_collection_is_404() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.get("/nonexistent").await.status(), 404);
}

#[tokio::test]
async fn get_unknown_collection_returns_error_json() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_json("/nonexistent").await;
    assert!(body.get("error").unwrap().is_string());
}

//   GET single item

#[tokio::test]
async fn get_item_by_id_returns_correct_item() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_json("/posts/1").await;
    assert_eq!(body.get("id").unwrap(), "1");
    assert_eq!(body.get("title").unwrap(), "Hello World");
    assert_eq!(body.get("author").unwrap(), "alice");
}

#[tokio::test]
async fn get_item_status_200() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.get("/posts/2").await.status(), 200);
}

#[tokio::test]
async fn get_missing_item_is_404() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.get("/posts/9999").await.status(), 404);
}

#[tokio::test]
async fn get_item_from_wrong_collection_is_404() {
    let s = TestServer::new(fixture_db()).await;
    // id "1" exists in posts but not comments with that id = 1 across resources
    assert_eq!(s.get("/tags/99").await.status(), 404);
}

// POST create
#[tokio::test]
async fn post_returns_201() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.post("/posts", json!({ "title": "New" })).await;
    assert_eq!(res.status(), 201);
}

#[tokio::test]
async fn post_returns_created_item() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.post_json("/posts", json!({ "title": "New", "author": "dave" })).await;
    assert_eq!(body.get("title").unwrap(), "New");
    assert_eq!(body.get("author").unwrap(), "dave");
}

#[tokio::test]
async fn post_auto_generates_non_empty_id() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.post_json("/posts", json!({ "title": "Auto ID" })).await;
    let id = body.get("id").unwrap().as_str().unwrap();
    assert!(!id.is_empty());
}

#[tokio::test]
async fn post_id_is_always_a_string() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.post_json("/posts", json!({ "title": "t" })).await;
    assert!(
        body.get("id").unwrap().is_string(),
        "id must be a JSON string, got {:?}",
        body.get("id").unwrap()
    );
}

#[tokio::test]
async fn post_with_explicit_string_id_keeps_it() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.post_json("/posts", json!({ "id": "custom-99", "title": "t" })).await;
    assert_eq!(body.get("id").unwrap(), "custom-99");
}

#[tokio::test]
async fn post_with_numeric_id_coerces_to_string() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.post_json("/posts", json!({ "id": 42, "title": "t" })).await;
    // Spec: ids are always strings
    assert_eq!(body.get("id").unwrap(), &json!("42"));
}

#[tokio::test]
async fn posted_item_is_retrievable() {
    let s = TestServer::new(fixture_db()).await;
    let created = s.post_json("/posts", json!({ "title": "Persisted?" })).await;
    let id = created.get("id").unwrap().as_str().unwrap();
    let fetched = s.get_json(&format!("/posts/{id}")).await;
    assert_eq!(fetched.get("title").unwrap(), "Persisted?");
}

#[tokio::test]
async fn post_to_new_resource_auto_creates_collection() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.post("/fruits", json!({ "name": "mango" })).await;
    assert_eq!(res.status(), 201);
    let all = s.get_json("/fruits").await;
    assert_eq!(all.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn post_to_new_resource_lists_in_root() {
    let s = TestServer::new(fixture_db()).await;
    s.post("/widgets", json!({ "color": "red" })).await;
    let root = s.get_json("/").await;
    let resources: Vec<&str> = root
        .get("resources")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(resources.contains(&"widgets"));
}

// PUT replace
// ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn put_returns_200() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.put("/posts/1", json!({ "title": "Replaced" })).await.status(), 200);
}

#[tokio::test]
async fn put_replaces_all_fields() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.put_json("/posts/1", json!({ "title": "Only Title" })).await;
    assert_eq!(body.get("title").unwrap(), "Only Title");
    // author and views must be gone — it's a full replace
    assert!(body.get("author").is_none(), "author should be absent after full replace");
    assert!(body.get("views").is_none(), "views should be absent after full replace");
}

#[tokio::test]
async fn put_url_id_wins_over_body_id() {
    let s = TestServer::new(fixture_db()).await;
    // Even if the body sends a different id, the URL id must be used
    let body = s.put_json("/posts/2", json!({ "id": "999", "title": "t" })).await;
    assert_eq!(body.get("id").unwrap(), "2");
}

#[tokio::test]
async fn put_change_is_persisted() {
    let s = TestServer::new(fixture_db()).await;
    s.put("/posts/3", json!({ "title": "Updated" })).await;
    let fetched = s.get_json("/posts/3").await;
    assert_eq!(fetched.get("title").unwrap(), "Updated");
}

#[tokio::test]
async fn put_missing_item_is_404() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.put("/posts/9999", json!({ "title": "Ghost" })).await.status(), 404);
}

// PATCH partial update
// ──────────────────────────────────────────────────────

#[tokio::test]
async fn patch_returns_200() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.patch("/posts/1", json!({ "views": 99 })).await.status(), 200);
}

#[tokio::test]
async fn patch_merges_new_field() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.patch_json("/posts/1", json!({ "featured": true })).await;
    assert_eq!(body.get("featured").unwrap(), true);
    // existing fields must survive
    assert_eq!(body.get("title").unwrap(), "Hello World");
    assert_eq!(body.get("author").unwrap(), "alice");
}

#[tokio::test]
async fn patch_updates_existing_field() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.patch_json("/posts/2", json!({ "views": 9999 })).await;
    assert_eq!(body.get("views").unwrap(), 9999);
    assert_eq!(body.get("title").unwrap(), "Rust is Fast"); // unrelated field unchanged
}

#[tokio::test]
async fn patch_cannot_change_id() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.patch_json("/posts/1", json!({ "id": "hacked" })).await;
    assert_eq!(body.get("id").unwrap(), "1", "id must be immutable");
}

#[tokio::test]
async fn patch_change_is_persisted() {
    let s = TestServer::new(fixture_db()).await;
    s.patch("/posts/4", json!({ "pinned": true })).await;
    let fetched = s.get_json("/posts/4").await;
    assert_eq!(fetched.get("pinned").unwrap(), true);
}

#[tokio::test]
async fn patch_missing_item_is_404() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.patch("/posts/9999", json!({ "x": 1 })).await.status(), 404);
}

// DELETE
#[tokio::test]
async fn delete_returns_no_content() {
    let s = TestServer::new(fixture_db()).await;
    let response = s.delete("/posts/1").await;
    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn deleted_item_is_gone() {
    let s = TestServer::new(fixture_db()).await;
    s.delete("/posts/2").await;
    assert_eq!(s.get("/posts/2").await.status(), 404);
}

#[tokio::test]
async fn delete_reduces_collection_count() {
    let s = TestServer::new(fixture_db()).await;
    s.delete("/posts/1").await;
    let all = s.get_json("/posts").await;
    assert_eq!(all.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn delete_missing_item_is_404() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.delete("/posts/9999").await.status(), 404);
}

#[tokio::test]
async fn delete_other_items_unaffected() {
    let s = TestServer::new(fixture_db()).await;
    s.delete("/posts/1").await;
    // items 2 3 4 must still exist
    for id in ["2", "3", "4"] {
        assert_eq!(s.get(&format!("/posts/{id}")).await.status(), 200, "post {id} should survive");
    }
}

// Cascading deletes
#[tokio::test]
async fn cascade_delete_removes_dependent_items() {
    let s = TestServer::new(cascade_db()).await;
    // post 1 has comments 1 and 2
    s.delete_qs("/posts/1", "_dependent=comments").await;

    let remaining = s.get_json("/comments").await;
    let comment_ids = ids(&remaining);
    assert!(!comment_ids.contains(&"1"), "comment 1 should be cascaded");
    assert!(!comment_ids.contains(&"2"), "comment 2 should be cascaded");
}

#[tokio::test]
async fn cascade_delete_keeps_unrelated_dependents() {
    let s = TestServer::new(cascade_db()).await;
    s.delete_qs("/posts/1", "_dependent=comments").await;

    let remaining = s.get_json("/comments").await;
    let comment_ids = ids(&remaining);
    // comment 3 belongs to post 2, must survive
    assert!(comment_ids.contains(&"3"), "comment 3 (for post 2) must survive");
}

#[tokio::test]
async fn cascade_delete_removes_parent() {
    let s = TestServer::new(cascade_db()).await;
    s.delete_qs("/posts/1", "_dependent=comments").await;
    assert_eq!(s.get("/posts/1").await.status(), 404, "parent post must be deleted");
}

#[tokio::test]
async fn delete_without_dependent_leaves_children() {
    let s = TestServer::new(cascade_db()).await;
    // Plain delete — no cascade
    s.delete("/posts/1").await;
    let comments = s.get_json("/comments").await;
    // All 3 comments still present (orphaned, but that's the caller's choice)
    assert_eq!(comments.as_array().unwrap().len(), 3);
}

// Int id strategy
#[tokio::test]
async fn int_ids_start_at_one_for_empty_collection() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Int).await;
    let body = s.post_json("/items", json!({ "name": "first" })).await;
    assert_eq!(body.get("id").unwrap(), "1");
}

#[tokio::test]
async fn int_ids_auto_increment_sequentially() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Int).await;
    let a = s.post_json("/items", json!({ "name": "a" })).await;
    let b = s.post_json("/items", json!({ "name": "b" })).await;
    let c = s.post_json("/items", json!({ "name": "c" })).await;
    let id_a: u64 = a.get("id").unwrap().as_str().unwrap().parse().unwrap();
    let id_b: u64 = b.get("id").unwrap().as_str().unwrap().parse().unwrap();
    let id_c: u64 = c.get("id").unwrap().as_str().unwrap().parse().unwrap();
    assert_eq!(id_b, id_a + 1);
    assert_eq!(id_c, id_b + 1);
}

#[tokio::test]
async fn int_ids_continue_from_existing_max() {
    // Existing max id is 5 — next must be 6
    let s = TestServer::with_strategy(
        json!({ "items": [{ "id": "3" }, { "id": "5" }, { "id": "1" }] }),
        IdStrategy::Int,
    )
    .await;
    let body = s.post_json("/items", json!({ "name": "x" })).await;
    assert_eq!(body.get("id").unwrap(), "6");
}

#[tokio::test]
async fn int_ids_are_always_strings() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Int).await;
    let body = s.post_json("/items", json!({ "name": "x" })).await;
    assert!(body.get("id").unwrap().is_string(), "Int strategy must still store id as string");
}

#[tokio::test]
async fn int_id_explicit_in_post_body_is_respected() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Int).await;
    let body = s.post_json("/items", json!({ "id": "99", "name": "x" })).await;
    assert_eq!(body.get("id").unwrap(), "99");
    // next auto-id must continue from 99
    let next = s.post_json("/items", json!({ "name": "y" })).await;
    assert_eq!(next.get("id").unwrap(), "100");
}

//  UUIDv7 id strategy
#[tokio::test]
async fn uuidv7_ids_are_lexicographically_ordered() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Uuidv7).await;
    let a = s.post_json("/items", json!({ "n": 1 })).await;
    let b = s.post_json("/items", json!({ "n": 2 })).await;
    let c = s.post_json("/items", json!({ "n": 3 })).await;

    let id_a = a.get("id").unwrap().as_str().unwrap();
    let id_b = b.get("id").unwrap().as_str().unwrap();
    let id_c = c.get("id").unwrap().as_str().unwrap();
    // UUIDv7 is time-sortable — lexicographic order == insertion order
    assert!(id_a < id_b, "v7 ids must be lexicographically increasing");
    assert!(id_b < id_c, "v7 ids must be lexicographically increasing");
}

#[tokio::test]
async fn uuidv7_ids_are_valid_uuid_format() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Uuidv7).await;
    let body = s.post_json("/items", json!({ "n": 1 })).await;
    let id = body.get("id").unwrap().as_str().unwrap();
    // UUID format: 8-4-4-4-12 hex chars
    assert!(
        id.len() == 36 && id.chars().filter(|&c| c == '-').count() == 4,
        "expected UUID format, got {id}"
    );
}

// UUIDv4 id strategy
#[tokio::test]
async fn uuidv4_ids_are_valid_uuid_format() {
    let s = TestServer::with_strategy(json!({ "items": [] }), IdStrategy::Uuidv4).await;
    let body = s.post_json("/items", json!({ "n": 1 })).await;
    let id = body.get("id").unwrap().as_str().unwrap();
    assert!(
        id.len() == 36 && id.chars().filter(|&c| c == '-').count() == 4,
        "expected UUID format, got {id}"
    );
}
