//! Query parameter parsing and application.
//!
//! Supports the full json-server v1 query spec:
//!
//! ## Filtering
//! - `field=value`               — exact equality
//! - `field:eq=value`            — explicit equality
//! - `field:ne=value`            — not equal
//! - `field:gt=value`            — greater than
//! - `field:gte=value`           — greater than or equal
//! - `field:lt=value`            — less than
//! - `field:lte=value`           — less than or equal
//! - `field:in=a,b,c`            — value in list
//! - `field:nin=a,b,c`           — value not in list
//! - `field:contains=substr`     — string contains (case-insensitive)
//! - `field:startsWith=prefix`   — string starts with
//! - `field:endsWith=suffix`     — string ends with
//! - `field:matches=regex`       — regex
//! - Nested paths: `author.name:eq=typicode`
//!
//! ## Sorting
//! `_sort=field,-other_field`    — comma-separated; `-` prefix = descending
//!
//! ## Pagination
//! `_page=1&_per_page=10`        — page-based pagination
//! `_start=0&_end=10`            — slice-based pagination
//! `_start=0&_limit=10`          — slice with limit
//!
//! ## Embedding relations
//! `_embed=comments`             — embed child records (hasMany)
//! `_expand=author`              — embed parent record (belongsTo)

use core::cmp::Ordering;
use core::convert::Infallible;
use core::str::FromStr;
use std::collections::HashMap;

use serde_json::Value;

#[derive(Clone, Debug, Default)]
pub struct QueryParams {
    pub filters: Vec<Filter>,
    pub sort: Vec<SortField>,
    pub pagination: Pagination,
    pub embed: Vec<String>,
    pub expand: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Filter {
    pub path: Vec<String>, // nested path split by '.'
    pub operator: Operator,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Nin,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
}
impl FromStr for Operator {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let op = match s.to_lowercase().trim() {
            "eq" => Operator::Eq,
            "ne" => Operator::Ne,
            "gt" => Operator::Gt,
            "gte" => Operator::Gte,
            "lt" => Operator::Lt,
            "lte" => Operator::Lte,
            "in" => Operator::In,
            "nin" => Operator::Nin,
            "contains" => Operator::Contains,
            "startswith" => Operator::StartsWith,
            "endswith" => Operator::EndsWith,
            "matches" => Operator::Matches,
            _ => Operator::Eq, // Fallback
        };
        Ok(op)
    }
}

#[derive(Clone, Debug)]
pub struct SortField {
    pub path: Vec<String>,
    pub descending: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Pagination {
    #[default]
    None,
    Page {
        page: usize,
        per_page: usize,
    },
    Slice {
        start: usize,
        end: Option<usize>,
        limit: Option<usize>,
    },
}

#[derive(Clone)]
pub struct PaginatedResult {
    pub data: Vec<Value>,
    pub total: usize,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub pages: Option<usize>,
    pub first: Option<usize>,
    pub prev: Option<usize>,
    pub next: Option<usize>,
    pub last: Option<usize>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Parsing
// ──────────────────────────────────────────────────────────────────────────────

const RESERVED_PARAMS: &[&str] =
    &["_sort", "_page", "_per_page", "_start", "_end", "_limit", "_embed", "_expand"];

#[expect(clippy::else_if_without_else)]
pub fn parse_query(raw: &HashMap<String, String>, default_per_page: usize) -> QueryParams {
    let mut params = QueryParams::default();

    // Sorting
    if let Some(sort) = raw.get("_sort") {
        for part in sort.split(',') {
            let part = part.trim();

            let (descending, field) =
                if part.starts_with('-') { (true, &part[1..]) } else { (false, part) };
            params
                .sort
                .push(SortField { path: field.split('.').map(String::from).collect(), descending });
        }
    }

    // Pagination — page-based
    if let Some(page) = raw.get("_page") {
        let page = page.parse().unwrap_or(1).max(1);
        let per_page =
            raw.get("_per_page").and_then(|s| s.parse().ok()).unwrap_or(default_per_page);
        params.pagination = Pagination::Page { page, per_page };
    } else if raw.contains_key("_start") || raw.contains_key("_limit") {
        // Slice-based
        let start = raw.get("_start").and_then(|s| s.parse().ok()).unwrap_or(0);
        let end = raw.get("_end").and_then(|s| s.parse().ok());
        let limit = raw.get("_limit").and_then(|s| s.parse().ok());
        params.pagination = Pagination::Slice { start, end, limit };
    }

    // Embeds / expands
    if let Some(embed) = raw.get("_embed") {
        params.embed = embed.split(',').map(|s| s.trim().to_string()).collect();
    }

    if let Some(expand) = raw.get("_expand") {
        params.expand = expand.split(',').map(|s| s.trim().to_string()).collect();
    }

    // Filters — every non-reserved param
    for (key, value) in raw {
        if RESERVED_PARAMS.contains(&key.as_str()) {
            continue;
        }

        // Detect `field:operator` syntax
        if let Some(colon_pos) = key.find(':') {
            let field = &key[..colon_pos];
            let op = &key[colon_pos + 1..];
            let Ok(operator) = op.parse();

            params.filters.push(Filter {
                path: field.split('.').map(String::from).collect(),
                operator,
                value: value.to_owned(),
            });
        } else {
            // Plain equality filter
            params.filters.push(Filter {
                path: key.split('.').map(String::from).collect(),
                operator: Operator::Eq,
                value: value.to_owned(),
            });
        }
    }

    params
}

// ──────────────────────────────────────────────────────────────────────────────
// Application
// ──────────────────────────────────────────────────────────────────────────────

pub fn apply_query(items: Vec<Value>, params: &QueryParams) -> PaginatedResult {
    // 1. Filter
    let mut items: Vec<Value> =
        items.into_iter().filter(|item| apply_filters(item, &params.filters)).collect();

    // 2. Sort
    if !params.sort.is_empty() {
        items.sort_by(|a, b| {
            for sort_field in &params.sort {
                let va = get_path(a, sort_field.path.iter());
                let vb = get_path(b, sort_field.path.iter());
                let cmp = compare_values(&va, &vb);
                let cmp = if sort_field.descending { cmp.reverse() } else { cmp };
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            Ordering::Equal
        });
    }

    let total = items.len();

    // 3. Paginate
    match &params.pagination {
        Pagination::None => PaginatedResult {
            data: items,
            total,
            page: None,
            per_page: None,
            pages: None,
            first: None,
            prev: None,
            next: None,
            last: None,
        },

        Pagination::Page { page, per_page } => {
            let pages = if *per_page == 0 { 1 } else { (total + per_page - 1) / per_page };
            let start = (*page - 1) * per_page;
            let data: Vec<Value> = items.into_iter().skip(start).take(*per_page).collect();

            PaginatedResult {
                data,
                total,
                page: Some(*page),
                per_page: Some(*per_page),
                pages: Some(pages),
                first: Some(1),
                prev: if *page > 1 { Some(*page - 1) } else { None },
                next: if *page < pages { Some(*page + 1) } else { None },
                last: Some(pages),
            }
        }

        Pagination::Slice { start, end, limit } => {
            let start = (*start).min(total);
            let end = match (end, limit) {
                (Some(e), _) => (*e).min(total),
                (None, Some(l)) => (start + l).min(total),
                (None, None) => total,
            };
            let data: Vec<Value> = items.into_iter().skip(start).take(end - start).collect();
            PaginatedResult {
                data,
                total,
                page: None,
                per_page: None,
                pages: None,
                first: None,
                prev: None,
                next: None,
                last: None,
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Filter evaluation
// ──────────────────────────────────────────────────────────────────────────────

fn apply_filters(item: &Value, filters: &[Filter]) -> bool {
    filters.iter().all(|f| apply_filter(item, f))
}

fn apply_filter(item: &Value, filter: &Filter) -> bool {
    let field_val = get_path(item, filter.path.iter());

    match filter.operator {
        Operator::Eq => value_eq(&field_val, &filter.value),
        Operator::Ne => !value_eq(&field_val, &filter.value),
        Operator::Gt => compare_to_string(&field_val, &filter.value) == Some(Ordering::Greater),
        Operator::Gte => matches!(
            compare_to_string(&field_val, &filter.value),
            Some(Ordering::Greater | Ordering::Equal)
        ),
        Operator::Lt => compare_to_string(&field_val, &filter.value) == Some(Ordering::Less),
        Operator::Lte => matches!(
            compare_to_string(&field_val, &filter.value),
            Some(Ordering::Less | Ordering::Equal)
        ),
        Operator::In => filter.value.split(',').any(|v| value_eq(&field_val, v.trim())),
        Operator::Nin => !filter.value.split(',').any(|v| value_eq(&field_val, v.trim())),
        Operator::Contains => value_as_str(&field_val)
            .map(|s| s.to_lowercase().contains(&filter.value.to_lowercase()))
            .unwrap_or(false),
        Operator::StartsWith => value_as_str(&field_val)
            .map(|s| s.to_lowercase().starts_with(&filter.value.to_lowercase()))
            .unwrap_or(false),
        Operator::EndsWith => value_as_str(&field_val)
            .map(|s| s.to_lowercase().ends_with(&filter.value.to_lowercase()))
            .unwrap_or(false),
        Operator::Matches => {
            // Very basic regex via stdlib; for full regex add the `regex` crate
            value_as_str(&field_val).map(|s| simple_match(&s, &filter.value)).unwrap_or(false)
        }
    }
}

/// Walk a dotted path into a JSON value.
fn get_path<'a>(item: &'a Value, path: impl Iterator<Item = &'a String>) -> Option<&'a Value> {
    let mut current = item;
    for key in path {
        current = current.get(key)?;
    }
    Some(current)
}

fn value_as_str(val: &Option<&Value>) -> Option<String> {
    val.map(|v| match v {
        Value::String(s) => s.to_owned(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        _ => v.to_string(),
    })
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_owned(),
        Value::Number(n) => n.to_string(),
        _ => v.to_string(),
    }
}

fn value_eq(field: &Option<&Value>, target: &str) -> bool {
    match field {
        None => false,
        Some(Value::String(s)) => s == target,
        Some(Value::Number(n)) => n.to_string() == target,
        Some(Value::Bool(b)) => b.to_string() == target,
        Some(Value::Null) => target == "null",
        Some(v) => v.to_string() == target,
    }
}

fn compare_to_string(field: &Option<&Value>, target: &str) -> Option<Ordering> {
    let (f_num, t_num) = (field.and_then(|v| v.as_f64()), target.parse::<f64>().ok());

    match (f_num, t_num) {
        (Some(f), Some(t)) => f.partial_cmp(&t),
        _ => {
            // Fall back to lexicographic comparison
            let fs = value_as_str(field)?;
            Some(fs.as_str().cmp(target))
        }
    }
}

fn compare_values(a: &Option<&Value>, b: &Option<&Value>) -> Ordering {
    let (a_num, b_num) = (a.and_then(|v| v.as_f64()), b.and_then(|v| v.as_f64()));

    match (a_num, b_num) {
        (Some(an), Some(bn)) => an.partial_cmp(&bn).unwrap_or(Ordering::Equal),
        _ => {
            let a = value_as_str(a).unwrap_or_default();
            let b = value_as_str(b).unwrap_or_default();
            a.cmp(&b)
        }
    }
}

/// Naive wildcard match (* and ?) — avoids adding regex crate dependency.
/// Users can add the `regex` crate for full regex support.
fn simple_match(s: &str, pattern: &str) -> bool {
    // If pattern has no wildcards, treat as substring search (case-insensitive)
    if !pattern.contains('*') && !pattern.contains('?') {
        return s.to_lowercase().contains(&pattern.to_lowercase());
    }
    wildcard_match(s, pattern)
}

fn wildcard_match(s: &str, pattern: &str) -> bool {
    let s: Vec<char> = s.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let mut dp = vec![vec![false; p.len() + 1]; s.len() + 1];
    dp[0][0] = true;
    for j in 1..=p.len() {
        if p[j - 1] == '*' {
            dp[0][j] = dp[0][j - 1];
        }
    }

    for i in 1..=s.len() {
        for j in 1..=p.len() {
            dp[i][j] = match p[j - 1] {
                '*' => dp[i - 1][j] || dp[i][j - 1],
                '?' => dp[i - 1][j - 1],
                c => {
                    dp[i - 1][j - 1]
                        && (c == s[i - 1]
                            || c.to_lowercase().next() == s[i - 1].to_lowercase().next())
                }
            };
        }
    }
    dp[s.len()][p.len()]
}

// ──────────────────────────────────────────────────────────────────────────────
// Embedding
// ──────────────────────────────────────────────────────────────────────────────

/// Embed child collections (hasMany).
/// e.g. `_embed=comments` on `/posts/{id}` → each post gets a `comments` array
/// containing comments where `postId` == the post's id.
pub fn embed_children(
    items: &mut Vec<Value>,
    data: &serde_json::Map<String, Value>,
    embed: impl Iterator<Item = String>,
) {
    for collection_name in embed {
        let children = match data.get(&collection_name) {
            Some(Value::Array(arr)) => arr.clone(),
            _ => continue,
        };

        for item in items.iter_mut() {
            let item_id = match item.get("id") {
                Some(v) => value_to_string(v),
                None => continue,
            };

            // Convention: foreignKey = singularised collection + "Id"
            // e.g. posts → postId, comments → commentId
            // We also try the resource name without trailing 's' (naïve singularise)
            let fk_candidates = fk_candidates_for(&collection_name);

            let embedded: Vec<Value> = children
                .iter()
                .filter(|child| {
                    fk_candidates.iter().any(|fk| {
                        child.get(fk).map(|v| value_to_string(v) == item_id).unwrap_or(false)
                    })
                })
                .cloned()
                .collect();

            if let Some(obj) = item.as_object_mut() {
                obj.insert(collection_name.clone(), Value::Array(embedded));
            }
        }
    }
}

/// Expand a parent record (belongsTo).
/// e.g. `_expand=user` on `/posts` → each post with `userId` gets a `user`
/// field containing the matching user object.
pub fn expand_parents(
    items: &mut Vec<Value>,
    all_data: &serde_json::Map<String, Value>,
    expand: impl Iterator<Item = String>,
) {
    for parent_name in expand {
        // Find the parent collection (try singular, then plural)
        let parent_collection = find_parent_collection(all_data, &parent_name);
        let parent_arr = match parent_collection {
            Some(Value::Array(arr)) => arr.clone(),
            _ => continue,
        };

        let fk = format!("{}Id", parent_name);

        for item in items.iter_mut() {
            let parent_id = match item.get(&fk) {
                Some(v) => value_to_string(v),
                None => continue,
            };

            let parent = parent_arr
                .iter()
                .find(|p| p.get("id").map(|v| value_to_string(v) == parent_id).unwrap_or(false))
                .cloned();

            if let (Some(parent), Some(obj)) = (parent, item.as_object_mut()) {
                obj.insert(parent_name.to_owned(), parent);
            }
        }
    }
}

fn fk_candidates_for(collection: &str) -> Vec<String> {
    let singular = if collection.ends_with("ies") {
        format!("{}yId", &collection[..collection.len() - 3])
    } else if collection.ends_with('s') {
        format!("{}Id", &collection[..collection.len() - 1])
    } else {
        format!("{}Id", collection)
    };
    vec![singular, format!("{}Id", collection)]
}

fn find_parent_collection<'a>(
    data: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Option<&'a Value> {
    // Try exact match, then plural
    data.get(name).or_else(|| data.get(&format!("{}s", name)))
}
