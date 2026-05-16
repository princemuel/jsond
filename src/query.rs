//! Query parameter parsing and application — full json-server v1 spec.
//!
//! ## Filter syntax: `field:op=value`
//! Operators: (none)/eq, ne, lt, lte, gt, gte, in, contains, startsWith,
//! endsWith Nested paths: `author.name:eq=typicode`
//!
//! ## Sort: `_sort=field,-other`
//! Comma-separated; `-` prefix = descending. Supports dotted paths.
//!
//! ## Pagination (mutually exclusive):
//! - Page-based:  `_page=1&_per_page=25`  → envelope {
//!   first,prev,next,last,pages,items,data }
//! - Slice-based: `_start=0&_end=10`       → plain array + X-Total-Count
//!   `_start=0&_limit=10`      → plain array + X-Total-Count
//!
//! ## Relations:
//! - `_embed=comments`  → inline child records (hasMany: items where childId ==
//!   parent id)
//! - `_embed=post`      → inline parent record (belongsTo: item where id ==
//!   child's postId)
//!
//! ## Complex filter:
//! `_where={"or":[{"views":{"gt":100}},{"author":{"name":{"lt":"m"}}}]}`
//! Overrides individual field filter params when valid JSON.

use core::cmp::Ordering;
use core::hash::BuildHasher;
use std::collections::HashMap;

use serde_json::{Map, Value};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct QueryParams {
    pub sort: Vec<SortKey>,
    pub page: Option<usize>,
    pub per_page: usize,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub limit: Option<usize>,
    pub search: Option<String>,
    pub filters: Vec<Filter>,
    pub r#where: Option<WhereExpr>,
    pub embed: Vec<String>,  // _embed: hasMany (children)
    pub expand: Vec<String>, // _expand: belongsTo (parent)
}

#[derive(Clone, Debug)]
pub struct SortKey {
    pub path: Vec<String>, // dotted path split into segments
    pub desc: bool,
}

#[derive(Clone, Debug)]
pub struct Filter {
    pub path: Vec<String>,
    pub op: Op,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
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
    // Matches,
}

/// Represents a `_where` expression node.
#[derive(Debug, Clone)]
pub enum WhereExpr {
    And(Vec<WhereExpr>),
    Or(Vec<WhereExpr>),
    /// Leaf: field path, operator, value string
    Cond(Filter),
}

// ── Parse ─────────────────────────────────────────────────────────────────────

pub fn parse<S: BuildHasher>(raw: &HashMap<String, String, S>) -> QueryParams {
    let mut qp = QueryParams { per_page: 10, ..Default::default() };

    for (k, v) in raw {
        match k.as_str() {
            "_sort" => qp.sort = parse_sort(v),
            "_page" => qp.page = v.parse().ok(),
            "_per_page" => qp.per_page = v.parse().unwrap_or(10).max(1),
            "_start" => qp.start = v.parse().ok(),
            "_end" => qp.end = v.parse().ok(),
            "_limit" => qp.limit = v.parse().ok(),
            "q" => qp.search = Some(v.to_owned()),
            "_embed" => qp.embed.extend(v.split(',').map(str::trim).map(String::from)),
            "_expand" => qp.expand.extend(v.split(',').map(str::trim).map(String::from)),
            "_where" => qp.r#where = parse_where(v),
            "_dependent" => {} // handled in the delete handler, not query
            key => {
                if let Some(filter) = parse_filter(key, v) {
                    qp.filters.push(filter);
                }
            }
        }
    }
    qp
}

/// `_sort=title,-views,author.name`
fn parse_sort(s: &str) -> Vec<SortKey> {
    s.split(',')
        .filter(|p| !p.is_empty())
        .map(|seg| {
            let segment = seg.trim();

            let (desc, field) = if let Some(stripped) = segment.strip_prefix('-') {
                (true, stripped)
            } else {
                (false, segment)
            };

            SortKey { desc, path: dotted(field) }
        })
        .collect()
}

/// `author.name:gte=foo`  or  `views=100` (no operator = eq)
fn parse_filter(key: &str, value: &str) -> Option<Filter> {
    // Split on `:` to separate path from operator
    let (path, op) =
        if let Some((path, op)) = key.split_once(':') { (path, parse_op(op)?) } else { (key, Op::Eq) };

    // Skip internal underscore params that sneak through
    if path.starts_with('_') {
        return None;
    }

    Some(Filter { path: dotted(path), op, value: value.to_owned() })
}

fn parse_op(s: &str) -> Option<Op> {
    Some(match s.to_lowercase().trim() {
        "eq" => Op::Eq,
        "ne" => Op::Ne,
        "gt" => Op::Gt,
        "gte" => Op::Gte,
        "lt" => Op::Lt,
        "lte" => Op::Lte,
        "in" => Op::In,
        "nin" => Op::Nin,
        "contains" => Op::Contains,
        "startswith" => Op::StartsWith,
        "endswith" => Op::EndsWith,
        // "matches" => Op::Matches,
        _ => return None,
    })
}

/// Parse `_where` JSON into a `WhereExpr` tree.
/// Shape: `{"or":[{"field":{"op":value}}, ...]}`  or  `{"and":[...]}`
/// Unknown / malformed JSON is silently ignored (filters fall back to params).
fn parse_where(s: &str) -> Option<WhereExpr> {
    let v: Value = serde_json::from_str(s).ok()?;
    json_to_expr(&v)
}

fn json_to_expr(v: &Value) -> Option<WhereExpr> {
    let obj = v.as_object()?;

    if let Some(arr) = obj.get("or").and_then(Value::as_array) {
        let children: Vec<_> = arr.iter().filter_map(json_to_expr).collect();
        return Some(WhereExpr::Or(children));
    }

    if let Some(arr) = obj.get("and").and_then(Value::as_array) {
        let children: Vec<_> = arr.iter().filter_map(json_to_expr).collect();
        return Some(WhereExpr::And(children));
    }

    // Otherwise treat as { field: { op: value } }
    // e.g. {"views": {"gt": 100}}  or  {"author": {"name": {"lt": "m"}}}
    let mut conds = Vec::new();
    collect_leaf_conds(obj, &mut Vec::new(), &mut conds);

    match conds.len() {
        0 => None,
        1 => Some(conds.remove(0)),
        _ => Some(WhereExpr::And(conds)),
    }
}

#[expect(clippy::doc_link_with_quotes)]
#[expect(clippy::pattern_type_mismatch)]
/// Recurse `{"a": {"b": {"gt": "m"}}}` into path=["a","b"], op=Gt, value="m"
fn collect_leaf_conds(obj: &Map<String, Value>, path: &mut Vec<String>, out: &mut Vec<WhereExpr>) {
    obj.iter().for_each(|(k, v)| match (parse_op(k), v) {
        (Some(op), value) => out.push(WhereExpr::Cond(Filter {
            path: path.clone(),
            op,
            value: match value {
                Value::String(s) => s.to_owned(),
                v => v.to_string(),
            },
        })),
        (None, Value::Object(child)) => {
            path.push(k.to_owned());
            collect_leaf_conds(child, path, out);
            path.pop();
        }
        _ => {}
    });
}
#[derive(Clone, Copy)]
pub enum Pagination {
    None,
    Page { page: usize, per_page: usize, total: usize },
    Slice { start: usize, total: usize },
}

pub struct QueryResult {
    pub items: Vec<Value>,
    pub pagination: Pagination,
}

#[must_use]
pub fn apply(mut items: Vec<Value>, qp: &QueryParams) -> QueryResult {
    // 1. Filtering — _where overrides individual field params if present
    if let Some(expr) = qp.r#where.as_ref() {
        items.retain(|item| eval_where(item, expr));
    } else {
        for f in &qp.filters {
            items.retain(|item| matches_filter(item, f));
        }
    }

    // 2. Full-text search
    if let Some(q) = qp.search.as_ref() {
        let q = q.to_lowercase();
        items.retain(|item| full_text(item, &q));
    }

    let total = items.len();

    // 3. Sort — stable, multi-key, dot-path aware
    if !qp.sort.is_empty() {
        items.sort_by(|a, b| {
            for sk in &qp.sort {
                let av = sortable(get_nested(a, &sk.path));
                let bv = sortable(get_nested(b, &sk.path));
                let ord = av.partial_cmp(&bv).unwrap_or(Ordering::Equal);
                if ord != Ordering::Equal {
                    return if sk.desc { ord.reverse() } else { ord };
                }
            }
            Ordering::Equal
        });
    }

    // 4. Pagination — page-based xor slice-based
    if qp.page.is_some() {
        let page = qp.page.unwrap_or(1).max(1);
        let per_page = qp.per_page;
        let start = (page - 1) * per_page;

        items = items.into_iter().skip(start).take(per_page).collect();

        QueryResult { items, pagination: Pagination::Page { page, per_page, total } }
    } else if qp.start.is_some() || qp.end.is_some() || qp.limit.is_some() {
        let start = qp.start.unwrap_or(0);
        let end = qp.end.unwrap_or_else(|| qp.limit.map_or(total, |limit| start + limit));
        let slice_len = end.saturating_sub(start).min(total.saturating_sub(start));

        items = items.into_iter().skip(start).take(slice_len).collect();

        QueryResult { items, pagination: Pagination::Slice { start, total } }
    } else {
        QueryResult { items, pagination: Pagination::None }
    }
}

// ── _where evaluation
fn eval_where(item: &Value, expr: &WhereExpr) -> bool {
    match *expr {
        WhereExpr::And(ref v) => v.iter().all(|exp| eval_where(item, exp)),
        WhereExpr::Or(ref v) => v.iter().any(|exp| eval_where(item, exp)),
        WhereExpr::Cond(Filter { op, ref path, ref value }) => {
            matches_op(get_nested(item, path), op, value)
        }
    }
}

//   Filter matching

fn matches_filter(item: &Value, f: &Filter) -> bool {
    matches_op(get_nested(item, &f.path), f.op, &f.value)
}

fn matches_op(val: Option<&Value>, op: Op, target: &str) -> bool {
    match op {
        Op::Eq => eq_str(val, target),
        Op::Ne => !eq_str(val, target),

        Op::Gt => cmp_val(val, target) == Ordering::Greater,
        Op::Gte => matches!(cmp_val(val, target), Ordering::Greater | Ordering::Equal),

        Op::Lt => cmp_val(val, target) == Ordering::Less,
        Op::Lte => matches!(cmp_val(val, target), Ordering::Less | Ordering::Equal),

        Op::In => {
            let mut targets = target.split(',').map(str::trim);
            targets.any(|t| eq_str(val, t))
        }
        Op::Nin => {
            let mut targets = target.split(',').map(str::trim);
            !targets.any(|t| eq_str(val, t))
        }

        Op::Contains => str_op(val, target, |s, t| s.contains(t)),
        Op::StartsWith => str_op(val, target, |s, t| s.starts_with(t)),
        Op::EndsWith => str_op(val, target, |s, t| s.ends_with(t)),
        // Op::Matches => unimplemented!(),
    }
}

fn eq_str(val: Option<&Value>, target: &str) -> bool {
    match val.cloned() {
        Some(Value::String(v)) => v == target,
        Some(Value::Number(v)) => v.to_string() == target,
        Some(Value::Bool(v)) => v.to_string() == target,
        Some(Value::Null) => target == "null",
        // TODO: should i add this? Some(v) => v.to_string() == target,
        _ => false,
    }
}

/// Returns negative/zero/positive: prefers numeric, falls back to string.
fn cmp_val(val: Option<&Value>, target: &str) -> Ordering {
    let to_f64 = |v: Option<&Value>| match v.cloned() {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    };

    if let (Some(a), Ok(b)) = (to_f64(val), target.parse()) {
        return a.partial_cmp(&b).unwrap_or(Ordering::Equal);
    }

    let a = val.map(Value::to_string).unwrap_or_default();
    a.as_str().cmp(target)
}

/// Case-insensitive string operation (contains / startsWith / endsWith).
fn str_op(val: Option<&Value>, target: &str, f: impl Fn(&str, &str) -> bool) -> bool {
    let t = target.to_lowercase();
    match val.cloned() {
        Some(Value::String(v)) => f(&v.to_lowercase(), &t),
        Some(v) => f(&v.to_string().to_lowercase(), &t),
        None => false,
    }
}

//   Full-text searchWW
fn full_text(v: &Value, q: &str) -> bool {
    match v.to_owned() {
        Value::Object(v) => v.values().any(|v| full_text(v, q)),
        Value::Array(v) => v.iter().any(|v| full_text(v, q)),
        Value::String(v) => v.to_lowercase().contains(q),
        v => v.to_string().to_lowercase().contains(q),
    }
}

// ── Nested field access

/// Walk a dotted path through a JSON value: `["author", "name"]` →
/// `item.author.name`
#[must_use]
pub fn get_nested<'a>(v: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(v, |cur, seg| cur.get(seg.as_str()))
}

fn dotted(s: &str) -> Vec<String> { s.split('.').map(String::from).collect() }

//   Sort key

#[derive(Clone, Copy, PartialEq)]
enum Sk<'a> {
    Num(f64),
    Str(&'a str),
    Null,
}

#[expect(clippy::pattern_type_mismatch)]
impl PartialOrd for Sk<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Num(a), Self::Num(b)) => a.partial_cmp(b),
            (Self::Str(a), Self::Str(b)) => Some(a.cmp(b)),
            (Self::Null, Self::Null) => Some(Ordering::Equal),
            (Self::Null, _) => Some(Ordering::Less),
            (_, Self::Null) => Some(Ordering::Greater),
            _ => Some(Ordering::Less),
        }
    }
}

#[expect(clippy::pattern_type_mismatch)]
fn sortable(v: Option<&Value>) -> Sk<'_> {
    match v {
        Some(Value::Number(v)) => v.as_f64().map_or(Sk::Null, Sk::Num),
        Some(Value::String(v)) => Sk::Str(v),
        // For arrays, objects, and other types, treat as Null for sorting
        _ => Sk::Null,
    }
}
