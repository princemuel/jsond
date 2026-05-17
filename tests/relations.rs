//! Relation embedding tests — `_embed` (hasMany), `_expand` (belongsTo),
//! singleton resources, and edge cases around unknown relations.
#![expect(clippy::tests_outside_test_module)]
pub mod common;
use common::{TestServer, fixture_db, ids};
use serde_json::{Value, json};

//  _embed hasMany
#[tokio::test]
async fn embed_attaches_children_array_to_each_item() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_embed=comments").await;
    let arr = body.as_array().unwrap();
    for post in arr {
        assert!(post["comments"].is_array(), "post {} missing 'comments' array", post["id"]);
    }
}

#[tokio::test]
async fn embed_matches_correct_children_by_foreign_key() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_embed=comments").await;
    let arr = body.as_array().unwrap();

    let post1 = arr.iter().find(|p| p["id"] == "1").unwrap();
    let comment_ids = ids(&post1["comments"]);
    // comments 1 and 2 have postId="1"
    assert!(comment_ids.contains(&"1"));
    assert!(comment_ids.contains(&"2"));
    assert_eq!(comment_ids.len(), 2, "post 1 must have exactly 2 comments");
}

#[tokio::test]
async fn embed_excludes_unrelated_children() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_embed=comments").await;
    let arr = body.as_array().unwrap();

    let post2 = arr.iter().find(|p| p["id"] == "2").unwrap();
    let comment_ids = ids(&post2["comments"]);
    // comment 3 belongs to post 2; comments 1, 2, 4 do not
    assert_eq!(comment_ids, vec!["3"]);
}

#[tokio::test]
async fn embed_returns_empty_array_when_no_children() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_embed=comments").await;
    let arr = body.as_array().unwrap();
    // post 4 has no comments at all
    let post4 = arr.iter().find(|p| p["id"] == "4").unwrap();
    assert_eq!(post4["comments"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn embed_on_single_item_get() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts/2", "_embed=comments").await;
    let comments = body.get("comments").unwrap().as_array().unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments.first().unwrap().get("id").unwrap(), "3");
}

#[tokio::test]
async fn embed_does_not_affect_unembedded_fields() {
    let s = TestServer::new(fixture_db()).await;
    let with_embed = s.get_qs_json("/posts", "_embed=comments").await;
    let without = s.get_json("/posts").await;
    let arr_with = with_embed.as_array().unwrap();
    let arr_without = without.as_array().unwrap();

    for (w, wo) in arr_with.iter().zip(arr_without.iter()) {
        assert_eq!(w["id"], wo["id"]);
        assert_eq!(w["title"], wo["title"]);
        assert_eq!(w["author"], wo["author"]);
        assert_eq!(w["views"], wo["views"]);
    }
}

#[tokio::test]
async fn embed_combined_with_filter() {
    let s = TestServer::new(fixture_db()).await;
    // Only alice's posts, with their comments
    let body = s.get_qs_json("/posts", "author=alice&_embed=comments").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    for post in arr {
        assert_eq!(post["author"], "alice");
        assert!(post["comments"].is_array());
    }
}

#[tokio::test]
async fn embed_combined_with_pagination() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_page=1&_per_page=2&_embed=comments").await;
    // Envelope shape
    let data = body.get("data").unwrap().as_array().unwrap();
    assert_eq!(data.len(), 2);
    for post in data {
        assert!(post["comments"].is_array());
    }
}

#[tokio::test]
async fn embed_unknown_child_resource_ignored() {
    let s = TestServer::new(fixture_db()).await;
    // "likes" doesn't exist — should not 404, just silently skip
    let res = s.get_qs("/posts", "_embed=likes").await;
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    // Posts still returned; no 'likes' key (or empty array, both acceptable)
    assert!(body.is_array());
}

// _expand belongsTo
#[tokio::test]
async fn expand_attaches_parent_object() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/comments", "_expand=post").await;
    let arr = body.as_array().unwrap();
    for comment in arr {
        assert!(comment["post"].is_object(), "comment {} missing 'post' object", comment["id"]);
    }
}

#[tokio::test]
async fn expand_matches_correct_parent_by_id() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/comments", "_expand=post").await;
    let arr = body.as_array().unwrap();

    for comment in arr {
        let post_id = comment["postId"].as_str().unwrap();
        let embedded_id = comment.get("post").unwrap().get("id").unwrap().as_str().unwrap();
        assert_eq!(post_id, embedded_id, "embedded post id must match comment's postId");
    }
}

#[tokio::test]
async fn expand_embeds_parent_fields() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/comments/3", "_expand=post").await;
    // Comment 3 has postId=2 → "Rust is Fast"
    assert_eq!(body.get("post").unwrap().get("id").unwrap(), "2");
    assert_eq!(body.get("post").unwrap().get("title").unwrap(), "Rust is Fast");
}

#[tokio::test]
async fn expand_on_single_item_get() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/comments/1", "_expand=post").await;
    assert_eq!(body.get("post").unwrap().get("id").unwrap(), "1");
    assert!(body.get("post").unwrap().is_object());
}

#[tokio::test]
async fn expand_combined_with_filter() {
    let s = TestServer::new(fixture_db()).await;
    // Comments with rating >= 5, expanded with their parent post
    let body = s.get_qs_json("/comments", "rating:gte=5&_expand=post").await;
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    for comment in arr {
        assert!(comment["rating"].as_i64().unwrap() >= 5);
        assert!(comment["post"].is_object());
    }
}

// Both _embed and _expand together

#[tokio::test]
async fn embed_and_expand_can_be_used_independently() {
    let s = TestServer::new(fixture_db()).await;
    // Embed tags onto posts (posts → tags)
    let with_tags = s.get_qs_json("/posts", "_embed=tags").await;
    let arr = with_tags.as_array().unwrap();
    for post in arr {
        assert!(post["tags"].is_array());
    }

    // Expand post onto tags (tags → post)
    let with_post = s.get_qs_json("/tags", "_expand=post").await;
    let arr = with_post.as_array().unwrap();
    for tag in arr {
        assert!(tag["post"].is_object() || tag["post"].is_null());
    }
}

//  Singleton resources

#[tokio::test]
async fn singleton_get_returns_object() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_json("/profile").await;
    assert!(body.is_object());
    assert_eq!(body.get("name").unwrap(), "admin");
    assert_eq!(body.get("email").unwrap(), "admin@example.com");
}

#[tokio::test]
async fn singleton_get_status_200() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.get("/profile").await.status(), 200);
}

#[tokio::test]
async fn singleton_put_replaces_entirely() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.put("/profile", json!({ "name": "root", "level": 99 })).await;
    assert_eq!(res.status(), 200);

    let body = s.get_json("/profile").await;
    assert_eq!(body.get("name").unwrap(), "root");
    assert_eq!(body.get("level").unwrap(), 99);
    // email must be gone — PUT is a full replace
    assert!(body.get("email").is_none(), "email must be absent after PUT");
}

#[tokio::test]
async fn singleton_put_returns_updated_object() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.put_json("/profile", json!({ "name": "root" })).await;
    assert_eq!(body.get("name").unwrap(), "root");
}

#[tokio::test]
async fn singleton_patch_merges_fields() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.patch("/profile", json!({ "department": "engineering" })).await;
    assert_eq!(res.status(), 200);

    let body = s.get_json("/profile").await;
    // New field added
    assert_eq!(body.get("department").unwrap(), "engineering");
    // Existing fields preserved
    assert_eq!(body.get("name").unwrap(), "admin");
    assert_eq!(body.get("email").unwrap(), "admin@example.com");
}

#[tokio::test]
async fn singleton_patch_overwrites_existing_field() {
    let s = TestServer::new(fixture_db()).await;
    s.patch("/profile", json!({ "name": "superadmin" })).await;
    let body = s.get_json("/profile").await;
    assert_eq!(body.get("name").unwrap(), "superadmin");
}

#[tokio::test]
async fn singleton_patch_returns_merged_object() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.patch_json("/profile", json!({ "x": 1 })).await;
    // Should contain both original fields and the new one
    assert_eq!(body.get("name").unwrap(), "admin");
    assert_eq!(body.get("x").unwrap(), 1);
}

#[tokio::test]
async fn singleton_put_on_collection_resource_is_404() {
    let s = TestServer::new(fixture_db()).await;
    // /posts is a collection — PUT /{resource} (singleton route) must 404
    let res = s.put("/posts", json!({ "x": 1 })).await;
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn singleton_patch_on_collection_resource_is_404() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.patch("/posts", json!({ "x": 1 })).await;
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn singleton_on_unknown_resource_is_404() {
    let s = TestServer::new(fixture_db()).await;
    assert_eq!(s.get("/settings").await.status(), 404);
}

//   Edge cases
#[tokio::test]
async fn collection_preserves_insertion_order_without_sort() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_json("/posts").await;
    // Should come back in file order: 1, 2, 3, 4
    assert_eq!(ids(&body), vec!["1", "2", "3", "4"]);
}

#[tokio::test]
async fn x_total_count_reflects_filtered_total_not_page() {
    let s = TestServer::new(fixture_db()).await;
    // 2 alice posts, page size 1 — X-Total-Count should be 2 (filtered total)
    let res = s.get_qs("/posts", "author=alice&_page=1&_per_page=1").await;
    assert_eq!(res.headers()["x-total-count"], "2");
}

#[tokio::test]
async fn multiple_sequential_writes_are_consistent() {
    let s = TestServer::new(json!({ "items": [] })).await;

    // POST 5 items
    for i in 0..5_u32 {
        s.post("/items", json!({ "n": i })).await;
    }

    let all = s.get_json("/items").await;
    assert_eq!(all.as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn write_read_write_read_is_consistent() {
    let s = TestServer::new(fixture_db()).await;

    // Create
    let created = s.post_json("/posts", json!({ "title": "Round-trip" })).await;
    let id = created.get("id").unwrap().as_str().unwrap();

    // Patch
    s.patch(&format!("/posts/{id}"), json!({ "views": 42 })).await;

    // Read back
    let fetched = s.get_json(&format!("/posts/{id}")).await;
    assert_eq!(fetched.get("title").unwrap(), "Round-trip");
    assert_eq!(fetched.get("views").unwrap(), 42);

    // Delete
    s.delete(&format!("/posts/{id}")).await;
    assert_eq!(s.get(&format!("/posts/{id}")).await.status(), 404);
}
