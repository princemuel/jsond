//! Query parameter tests. Filtering, sorting, pagination, full-text search,
//! and `_where` complex expressions.
#![expect(clippy::tests_outside_test_module)]
pub mod common;
use common::{TestServer, assert_sorted_asc, assert_sorted_desc, fixture_db, ids};
use serde_json::json;

// Exact filter (field=value and field:eq=value)
#[tokio::test]
async fn filter_exact_implicit_eq() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "author=alice").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr.iter().all(|p| p["author"] == "alice"));
}

#[tokio::test]
async fn filter_explicit_eq_operator() {
    let s = TestServer::new(fixture_db()).await;
    let implicit = s.get_qs_json("/posts", "author=alice").await;
    let explicit = s.get_qs_json("/posts", "author:eq=alice").await;
    assert_eq!(implicit, explicit, ":eq must produce same result as bare =");
}

#[tokio::test]
async fn filter_no_matches_returns_empty_array() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "author=nobody").await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn filter_multiple_params_are_anded() {
    let s = TestServer::new(fixture_db()).await;
    // alice has views=10 and views=80; only 80 passes :gt=50
    let body = s.get_qs_json("/posts", "author=alice&views:gt=50").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr.first().unwrap().get("id").unwrap(), "3");
}

// :ne
#[tokio::test]
async fn filter_ne() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "author:ne=alice").await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().all(|p| p["author"] != "alice"));
    // bob + carol = 2
    assert_eq!(arr.len(), 2);
}

// Numeric range operators. gt, gte, lt, lte
#[tokio::test]
async fn filter_gt() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "views:gt=100").await;
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    assert!(arr.iter().all(|p| p["views"].as_i64().unwrap() > 100));
}

#[tokio::test]
async fn filter_gte_includes_boundary() {
    let s = TestServer::new(fixture_db()).await;
    // views=250 is exactly the boundary
    let body = s.get_qs_json("/posts", "views:gte=250").await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().all(|p| p["views"].as_i64().unwrap() >= 250));
    assert!(arr.iter().any(|p| p["views"].as_i64().unwrap() == 250), "boundary must be included");
}

#[tokio::test]
async fn filter_lt() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "views:lt=100").await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().all(|p| p["views"].as_i64().unwrap() < 100));
}

#[tokio::test]
async fn filter_lte_includes_boundary() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "views:lte=80").await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().all(|p| p["views"].as_i64().unwrap() <= 80));
    assert!(arr.iter().any(|p| p["views"].as_i64().unwrap() == 80), "boundary must be included");
}

// :in
#[tokio::test]
async fn filter_in_matches_listed_values() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "id:in=1,3").await;
    let result_ids = ids(&body);
    assert_eq!(result_ids.len(), 2);
    assert!(result_ids.contains(&"1"));
    assert!(result_ids.contains(&"3"));
}

#[tokio::test]
async fn filter_in_excludes_unlisted_values() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "id:in=1,3").await;
    let result_ids = ids(&body);
    assert!(!result_ids.contains(&"2"));
    assert!(!result_ids.contains(&"4"));
}

#[tokio::test]
async fn filter_in_single_value() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "id:in=2").await;
    assert_eq!(ids(&body), vec!["2"]);
}

// String operators: contains, startsWith, endsWith
#[tokio::test]
async fn filter_contains_case_insensitive() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "title:contains=rust").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr.first().unwrap().get("title").unwrap(), "Rust is Fast");
}

#[tokio::test]
async fn filter_contains_uppercase_query() {
    let s = TestServer::new(fixture_db()).await;
    let lower = s.get_qs_json("/posts", "title:contains=rust").await;
    let upper = s.get_qs_json("/posts", "title:contains=RUST").await;
    assert_eq!(lower, upper, "contains must be case-insensitive");
}

#[tokio::test]
async fn filter_starts_with() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "title:startsWith=ax").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr.first().unwrap().get("title").unwrap(), "Axum is Ergonomic");
}

#[tokio::test]
async fn filter_starts_with_case_insensitive() {
    let s = TestServer::new(fixture_db()).await;
    let lower = s.get_qs_json("/posts", "title:startsWith=ax").await;
    let upper = s.get_qs_json("/posts", "title:startsWith=AX").await;
    assert_eq!(lower, upper);
}

#[tokio::test]
async fn filter_ends_with() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "title:endsWith=world").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr.first().unwrap().get("title").unwrap(), "Hello World");
}

#[tokio::test]
async fn filter_ends_with_case_insensitive() {
    let s = TestServer::new(fixture_db()).await;
    let lower = s.get_qs_json("/posts", "title:endsWith=world").await;
    let upper = s.get_qs_json("/posts", "title:endsWith=WORLD").await;
    assert_eq!(lower, upper);
}

// Nested dot-path filters
#[tokio::test]
async fn filter_nested_path() {
    let db = json!({
        "users": [
            { "id": "1", "address": { "city": "London" } },
            { "id": "2", "address": { "city": "Paris"  } },
            { "id": "3", "address": { "city": "London" } }
        ]
    });
    let s = TestServer::new(db).await;
    let body = s.get_qs_json("/users", "address.city=London").await;
    assert_eq!(ids(&body), vec!["1", "3"]);
}

#[tokio::test]
async fn filter_nested_path_with_operator() {
    let db = json!({
        "products": [
            { "id": "1", "meta": { "stock": 5  } },
            { "id": "2", "meta": { "stock": 50 } },
            { "id": "3", "meta": { "stock": 0  } }
        ]
    });
    let s = TestServer::new(db).await;
    let body = s.get_qs_json("/products", "meta.stock:gt=4").await;
    let result_ids = ids(&body);
    assert!(result_ids.contains(&"1"));
    assert!(result_ids.contains(&"2"));
    assert!(!result_ids.contains(&"3"));
}

// Full-text search
#[tokio::test]
async fn full_text_search_finds_matches() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "q=zero").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr.first().unwrap().get("id").unwrap(), "4");
}

#[tokio::test]
async fn full_text_search_is_case_insensitive() {
    let s = TestServer::new(fixture_db()).await;
    let lower = s.get_qs_json("/posts", "q=rust").await;
    let upper = s.get_qs_json("/posts", "q=RUST").await;
    assert_eq!(lower, upper);
}

#[tokio::test]
async fn full_text_search_scans_all_fields() {
    let s = TestServer::new(fixture_db()).await;
    // "alice" appears in the author field, not the title
    let body = s.get_qs_json("/posts", "q=alice").await;
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn full_text_search_no_match_returns_empty() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "q=xyzzy_not_found").await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

// Sorting
#[tokio::test]
async fn sort_numeric_ascending() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_sort=views").await;
    assert_sorted_asc(&body, "views");
}

#[tokio::test]
async fn sort_numeric_descending_with_minus_prefix() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_sort=-views").await;
    assert_sorted_desc(&body, "views");
}

#[tokio::test]
async fn sort_string_field_ascending() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_sort=author").await;
    let authors: Vec<_> =
        body.as_array().unwrap().iter().map(|p| p["author"].as_str().unwrap()).collect();
    let mut sorted = authors.clone();
    sorted.sort_unstable();
    assert_eq!(authors, sorted);
}

#[tokio::test]
async fn sort_multi_key_primary_then_secondary() {
    let s = TestServer::new(fixture_db()).await;
    // author asc, then views desc within the same author
    let body = s.get_qs_json("/posts", "_sort=author,-views").await;
    let arr = body.as_array().unwrap();

    // alice (views 80, 10) must appear before bob/carol
    assert_eq!(arr.first().unwrap().get("author").unwrap(), "alice");
    assert_eq!(arr.get(1).unwrap().get("author").unwrap(), "alice");
    // within alice, views descending: 80 first
    assert_eq!(arr.first().unwrap().get("views").unwrap(), 80);
    assert_eq!(arr.get(1).unwrap().get("views").unwrap(), 10);
}

#[tokio::test]
async fn sort_combined_with_filter() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "author=alice&_sort=-views").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    // alice: 80 then 10 in descending order
    assert_eq!(arr.first().unwrap().get("views").unwrap(), 80);
    assert_eq!(arr.get(1).unwrap().get("views").unwrap(), 10);
}

// Pagination via page
#[tokio::test]
async fn page_pagination_returns_envelope() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_page=1&_per_page=2").await;
    // Must have all spec envelope keys
    assert!(body.get("first").is_some(), "missing 'first'");
    assert!(body.get("last").is_some(), "missing 'last'");
    assert!(body.get("pages").is_some(), "missing 'pages'");
    assert!(body.get("items").is_some(), "missing 'items'");
    assert!(body.get("data").is_some(), "missing 'data'");
}

#[tokio::test]
async fn page_pagination_page1_metadata() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_page=1&_per_page=2").await;
    assert_eq!(body.get("first").unwrap(), 1);
    assert_eq!(body.get("last").unwrap(), 2); // 4 posts / 2 per_page = 2 pages
    assert_eq!(body.get("pages").unwrap(), 2);
    assert_eq!(body.get("items").unwrap(), 4); // total count
    assert!(body.get("prev").unwrap().is_null(), "page 1 must have null prev");
    assert_eq!(body.get("next").unwrap(), 2);
}

#[tokio::test]
async fn page_pagination_page2_metadata() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_page=2&_per_page=2").await;
    assert_eq!(body.get("prev").unwrap(), 1);
    assert!(body.get("next").unwrap().is_null(), "last page must have null next");
}

#[tokio::test]
async fn page_pagination_data_length() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_page=1&_per_page=2").await;
    assert_eq!(body.get("data").unwrap().as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn page_pagination_last_page_partial() {
    let s = TestServer::new(fixture_db()).await;
    // 4 items, 3 per page. page 2 has 1 item
    let body = s.get_qs_json("/posts", "_page=2&_per_page=3").await;
    assert_eq!(body.get("data").unwrap().as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn page_pagination_items_is_total_not_page_count() {
    let s = TestServer::new(fixture_db()).await;
    // Apply a filter first. items must reflect filtered total
    let body = s.get_qs_json("/posts", "author=alice&_page=1&_per_page=10").await;
    // alice has 2 posts; items should be 2, not 4
    assert_eq!(body.get("items").unwrap(), 2);
    assert_eq!(body.get("data").unwrap().as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn page_pagination_x_total_count_header() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.get_qs("/posts", "_page=1&_per_page=2").await;
    assert_eq!(res.headers()["x-total-count"], "4");
}

// Pagination via slice
#[tokio::test]
async fn slice_start_end_returns_plain_array() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_start=0&_end=2").await;
    assert!(body.is_array(), "_start/_end must return plain array, not envelope");
}

#[tokio::test]
async fn slice_start_end_correct_length() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_start=1&_end=3").await;
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn slice_start_limit_correct_length() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_start=0&_limit=3").await;
    assert_eq!(body.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn slice_has_x_total_count_header() {
    let s = TestServer::new(fixture_db()).await;
    let res = s.get_qs("/posts", "_start=0&_limit=2").await;
    // Total count is for unsliced result
    assert_eq!(res.headers()["x-total-count"], "4");
}

#[tokio::test]
async fn slice_start_only_goes_to_end_of_collection() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", "_start=2").await;
    // 4 total, start=2 => 2 items
    assert_eq!(body.as_array().unwrap().len(), 2);
}

// _where complex filters: and, or, dotted-paths
#[tokio::test]
async fn where_simple_condition() {
    let s = TestServer::new(fixture_db()).await;
    let body = s.get_qs_json("/posts", r#"_where={"views":{"gt":200}}"#).await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().all(|p| p["views"].as_i64().unwrap() > 200));
}

#[tokio::test]
async fn where_or_combines_results() {
    let s = TestServer::new(fixture_db()).await;
    // views > 400 OR author == "bob"
    let qs = r#"_where={"or":[{"views":{"gt":400}},{"author":{"eq":"bob"}}]}"#;
    let body = s.get_qs_json("/posts", qs).await;
    let arr = body.as_array().unwrap();
    // id=4 (500 views) + id=2 (bob)
    assert_eq!(arr.len(), 2);
    let result_ids = ids(&body);
    assert!(result_ids.contains(&"2"));
    assert!(result_ids.contains(&"4"));
}

#[tokio::test]
async fn where_and_narrows_results() {
    let s = TestServer::new(fixture_db()).await;
    let qs = r#"_where={"and":[{"views":{"gt":50}},{"views":{"lt":300}}]}"#;
    let body = s.get_qs_json("/posts", qs).await;
    let arr = body.as_array().unwrap();
    // views 80 (id=3) and 250 (id=2) — ids 1 (10) and 4 (500) are out
    assert_eq!(arr.len(), 2);
    let result_ids = ids(&body);
    assert!(result_ids.contains(&"2"));
    assert!(result_ids.contains(&"3"));
}

#[tokio::test]
async fn where_nested_path() {
    let db = json!({
        "users": [
            { "id": "1", "score": { "value": 90 } },
            { "id": "2", "score": { "value": 40 } },
            { "id": "3", "score": { "value": 75 } }
        ]
    });
    let s = TestServer::new(db).await;
    let qs = r#"_where={"score":{"value":{"gte":75}}}"#;
    let body = s.get_qs_json("/users", qs).await;
    let result_ids = ids(&body);
    assert!(result_ids.contains(&"1"));
    assert!(result_ids.contains(&"3"));
    assert!(!result_ids.contains(&"2"));
}

#[tokio::test]
async fn where_overrides_plain_filter_params() {
    let s = TestServer::new(fixture_db()).await;
    // Plain param says author=alice, _where says views > 400.
    // When _where is present it should override the plain param.
    // Result: only views > 400 (carol, id=4). not alice-filtered.
    let qs = r#"author=alice&_where={"views":{"gt":400}}"#;
    let body = s.get_qs_json("/posts", qs).await;
    let result_ids = ids(&body);
    // id=4 (carol, 500 views) passes _where; alice's posts (10, 80 views) do not
    assert_eq!(result_ids, vec!["4"]);
}

#[tokio::test]
async fn where_malformed_json_falls_back_to_plain_filters() {
    let s = TestServer::new(fixture_db()).await;
    // Malformed _where is ignored, plain author filter applies
    let qs = "_where=NOT_JSON&author=bob";
    let body = s.get_qs_json("/posts", qs).await;
    assert_eq!(ids(&body), vec!["2"]);
}

// Combined operations
#[tokio::test]
async fn filter_sort_and_slice_combined() {
    let s = TestServer::new(fixture_db()).await;
    // Filter to views > 0 (all), sort by views desc, take first 2
    let body = s.get_qs_json("/posts", "views:gt=0&_sort=-views&_limit=2").await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    // Top 2 by views desc: carol (500), bob (250)
    assert_eq!(arr.first().unwrap().get("id").unwrap(), "4");
    assert_eq!(arr.get(1).unwrap().get("id").unwrap(), "2");
}

#[tokio::test]
async fn search_with_sort_and_page() {
    let db = json!({
        "items": [
            { "id": "1", "name": "rust book",    "price": 30 },
            { "id": "2", "name": "rust cookbook", "price": 45 },
            { "id": "3", "name": "go handbook",   "price": 25 },
            { "id": "4", "name": "rust guide",    "price": 20 }
        ]
    });
    let s = TestServer::new(db).await;
    // search "rust", sort by price asc, page 1 of 2 per page
    let body = s.get_qs_json("/items", "q=rust&_sort=price&_page=1&_per_page=2").await;
    // 3 rust matches: ids 1,2,4. Sorted by price asc: 4(20), 1(30), 2(45). Page 1
    // of 2.
    let data = body.get("data").unwrap();
    assert_eq!(data.as_array().unwrap().len(), 2);
    assert_eq!(data.get(0).unwrap().get("id").unwrap(), "4"); // price 20
    assert_eq!(data.get(1).unwrap().get("id").unwrap(), "1"); // price 30
    // items = total filtered count
    assert_eq!(body.get("items").unwrap(), 3);
}
