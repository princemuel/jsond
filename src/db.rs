//! Thread-safe in-memory JSON database with atomic file persistence.
//!
//! Top-level keys are resource names.
//! - An array is a collection resource (GET / POST / PUT / PATCH / DELETE)
//! - An object is a singleton resource (GET / PUT / PATCH)
//!
//! Each created item gets an auto-generated ID whose format depends on the
//! resource id strategy is chosen at startup (uuidv4, uuidv7, or int).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::{debug, info};

use crate::error::Error;
use crate::id::ResourceId;

const EMPTY: &[Value] = &[];

#[derive(Clone)]
pub struct Database(Arc<RwLock<Inner>>);

pub struct Inner {
    pub data: Map<String, Value>,
    pub path: PathBuf,
    pub ids: ResourceId,
    pub readonly: bool,
}

impl Database {
    /// Load a database from a JSON or JSON5 file.
    pub fn load<P>(path: P, ids: ResourceId, readonly: bool) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref().to_path_buf();
        let content = fs::read_to_string(&path)?;
        let data = parse_db(&content, &path)?;
        Ok(Self(Arc::new(RwLock::new(Inner { data, path, ids, readonly }))))
    }

    /// Reload from disk (used by file watcher).
    pub async fn reload(&self) -> Result<(), Error> {
        let mut g = self.write().await;
        let content = fs::read_to_string(&g.path)?;

        g.data = parse_db(&content, &g.path)?;
        info!("Reloaded database from {}", g.path.display());
        Ok(())
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, Inner> { self.0.read().await }

    pub async fn write(&self) -> RwLockWriteGuard<'_, Inner> { self.0.write().await }

    /// Get the names of all top-level keys.
    pub async fn resources(&self) -> Vec<String> {
        self.read().await.data.keys().cloned().collect()
    }

    /// is the key an array (`collection`).
    pub async fn is_collection(&self, resource: &str) -> bool {
        matches!(self.read().await.data.get(resource), Some(Value::Array(_)))
    }

    /// is the key an object (`singleton`).
    pub async fn is_singleton(&self, resource: &str) -> bool {
        matches!(self.read().await.data.get(resource), Some(Value::Object(_)))
    }

    pub async fn is_resource(&self, resource: &str) -> bool {
        self.read().await.data.contains_key(resource)
    }

    /// Get a collection (array).
    pub async fn get_collection(&self, resource: &str) -> Option<Vec<Value>> {
        self.read().await.data.get(resource)?.as_array().cloned()
    }

    /// Get a singleton (object).
    pub async fn get_singleton(&self, resource: &str) -> Option<Value> {
        self.read()
            .await
            .data
            .get(resource)
            .filter(|v| v.is_object())
            .cloned()
    }

    /// Find a single item by its `id` field.
    pub async fn find(&self, resource: &str, id: &str) -> Option<Value> {
        self.read()
            .await
            .data
            .get(resource)?
            .as_array()?
            .iter()
            .find(|item| id_matches(item, id))
            .cloned()
    }

    /// Insert a new item, assigning a string id if one is not present.
    pub async fn insert(&self, resource: &str, mut item: Value) -> Result<Value, Error> {
        let mut g = self.write().await;

        if item.get("id").is_none() {
            let collection = g
                .data
                .get(resource)
                .and_then(Value::as_array)
                .map_or(EMPTY, |v| v);
            let id = g.ids.next_id(collection);

            item.as_object_mut()
                .ok_or_else(|| Error::BadRequest("body must be a JSON object".to_owned()))?
                .insert("id".to_owned(), id);
        } else {
            normalize_id(&mut item);
        }

        match g
            .data
            .entry(resource.to_owned())
            .or_insert_with(|| Value::Array(vec![]))
        {
            Value::Array(v) => v.push(item.clone()),
            _ => return Err(Error::NotACollection(resource.to_owned())),
        }

        persist(&g)?;
        Ok(item)
    }

    /// Full replace (PUT). Uses the id from the url in the body.
    pub async fn replace(&self, resource: &str, id: &str, mut item: Value) -> Result<Value, Error> {
        item.as_object_mut()
            .ok_or_else(|| Error::BadRequest("body must be a JSON object".to_owned()))?
            .insert("id".to_owned(), Value::String(id.to_owned()));

        let mut g = self.write().await;
        let arr = collection_mut(&mut g, resource)?;
        let slot = arr
            .iter_mut()
            .find(|i| id_matches(i, id))
            .ok_or(Error::NotFound)?;
        *slot = item.clone();

        persist(&g)?;
        Ok(item)
    }

    /// Partial update (PATCH). Merges; the `id` is immutable.
    pub async fn patch(&self, resource: &str, id: &str, item: Value) -> Result<Value, Error> {
        let mut payload = item
            .as_object()
            .ok_or_else(|| Error::BadRequest("body must be a JSON object".to_owned()))?
            .clone();

        payload.remove("id"); // id is immutable; never let it be patched in

        let mut g = self.write().await;
        let arr = collection_mut(&mut g, resource)?;
        let existing = arr
            .iter_mut()
            .find(|i| id_matches(i, id))
            .ok_or(Error::NotFound)?;

        if !existing.is_object() {
            return Err(Error::NotACollection(resource.to_owned()));
        }

        // RFC 7396 JSON Merge Patch: nulls delete keys, objects recurse, scalars
        // replace
        merge_json_rfc_7396(existing, &Value::Object(payload));
        let item = existing.clone();

        persist(&g)?;
        Ok(item)
    }

    /// Delete an item. also delete dependents if `dependent_resource` is given
    pub async fn delete(
        &self,
        resource: &str,
        id: &str,
        dependent: Option<&str>,
    ) -> Result<Value, Error> {
        let mut g = self.write().await;
        let arr = collection_mut(&mut g, resource)?;
        let pos = arr
            .iter()
            .position(|i| id_matches(i, id))
            .ok_or(Error::NotFound)?;
        let item = arr.remove(pos);

        // Cascade remove all items in `dependent` where `<resource_singular>Id == id`
        if let Some(key) = dependent {
            let fk = format!("{}Id", singular(resource));

            if let Some(Value::Array(v)) = g.data.get_mut(key) {
                v.retain(|item| !field_matches(item, &fk, id));
            }
        }

        persist(&g)?;
        Ok(item)
    }

    /// Replace a singleton entirely (PUT).
    pub async fn replace_singleton(&self, resource: &str, item: Value) -> Result<Value, Error> {
        let mut g = self.write().await;

        if !matches!(g.data.get(resource), Some(Value::Object(_))) {
            return Err(Error::NotFound);
        }
        if !item.is_object() {
            return Err(Error::BadRequest("replacement must be an object".into()));
        }

        g.data.insert(resource.to_owned(), item.clone());

        persist(&g)?;
        Ok(item)
    }

    /// Merge-patch a singleton (PATCH).
    pub async fn patch_singleton(&self, resource: &str, item: Value) -> Result<Value, Error> {
        let mut g = self.write().await;

        let Some(Value::Object(payload)) = g.data.get_mut(resource) else {
            return Err(Error::NotFound);
        };

        if !item.is_object() {
            return Err(Error::BadRequest("replacement must be an object".into()));
        }

        let Value::Object(p) = item else {
            return Err(Error::BadRequest("patch body must be an object".into()));
        };

        payload.extend(p);
        let item = Value::Object(payload.clone());

        persist(&g)?;
        Ok(item)
    }
}

use std::borrow::Cow;
/// Singularize a resource's name
///
/// `posts` → `post`, `comments` → `comment`, `babies` → `baby`, `people` →
/// `people` (no trailing s)
#[must_use]
pub fn singular(s: &str) -> Cow<'_, str> {
    // FIXME: Improve on this later. This is okay for now but pretty naive
    if let Some(stem) = s.strip_suffix("ies") {
        return Cow::Owned(format!("{stem}y"));
    }

    Cow::Borrowed(s.strip_suffix("s").unwrap_or(s))
}

/// Ensure the `id` field is stored as a string
pub fn normalize_id(item: &mut Value) {
    if let Some(obj) = item.as_object_mut()
        && let Some(id) = obj.remove("id")
    {
        let s = match id {
            Value::String(v) => v,
            v => v.to_string(),
        };
        obj.insert("id".to_owned(), Value::String(s));
    }
}

/// Treat a JSON string or number as a comparable id string.
/// Returns None for any other JSON type (object, array, bool, null).
#[must_use]
pub fn as_str_or_number_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Compare an id value (which may be Number or String) against a string.
#[must_use]
pub fn id_matches(item: &Value, id: &str) -> bool { field_matches(item, "id", id) }

/// Compare an field's value (which may be Number or String) against a string.
#[must_use]
pub fn field_matches(item: &Value, field: &str, id: &str) -> bool {
    match item.get(field) {
        Some(Value::String(v)) => v == id,
        Some(Value::Number(n)) => n.to_string() == id,
        _ => false,
    }
}

fn collection_mut<'a>(g: &'a mut Inner, resource: &'a str) -> Result<&'a mut Vec<Value>, Error> {
    match g.data.get_mut(resource) {
        Some(Value::Array(v)) => Ok(v),
        Some(_) => Err(Error::NotACollection(resource.to_owned())),
        None => Err(Error::NotFound),
    }
}

fn parse_db(raw: &str, path: &Path) -> Result<Map<String, Value>, Error> {
    let is_json5 = path.extension().and_then(|e| e.to_str()) == Some("json5");

    let value: Value = if is_json5 {
        json5::from_str(raw)
    } else {
        serde_json::from_str(raw).or_else(|_| json5::from_str(raw))
    }
    .map_err(|e| Error::BadRequest(e.to_string()))?;

    match value {
        Value::Object(map) => Ok(map),
        _ => Err(Error::BadRequest(
            "top-level JSON must be an object, e.g { \"posts\": [...] }".into(),
        )),
    }
}

fn persist(g: &Inner) -> Result<(), Error> {
    if g.readonly {
        return Ok(());
    }

    let tmp = g.path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&g.data)?;

    fs::write(&tmp, json)?;
    fs::rename(&tmp, &g.path)?;

    debug!("Persisted database to {}", g.path.display());
    Ok(())
}

/// Patch provided JSON document in place via JSON Merge Patch (RFC 7396).
///
/// # Example
///
/// ```rust
/// use serde_json::json;
///
/// use jsond::db::merge_json_rfc_7396;
///
/// let mut doc = json!({
///   "title": "Goodbye!",
///   "author" : { "givenName": "John", "familyName": "Doe" },
///   "tags":[ "example", "sample" ],
///   "content": "This will be unchanged"
/// });
///
/// let patch = json!({
///   "title": "Hello!",
///   "phoneNumber": "+01-123-456-7890",
///   "author": { "familyName": null },
///   "tags": [ "example" ]
/// });
///
/// merge_json_rfc_7396(&mut doc, &patch);
/// assert_eq!(doc, json!({
///   "title": "Hello!",
///   "author" : { "givenName": "John" },
///   "tags": [ "example" ],
///   "content": "This will be unchanged",
///   "phoneNumber": "+01-123-456-7890"
/// }));
/// ```
pub fn merge_json_rfc_7396(doc: &mut Value, patch: &Value) {
    let Some(patch_map) = patch.as_object() else {
        *doc = patch.clone();
        return;
    };

    let map = if let Some(map) = doc.as_object_mut() {
        map
    } else {
        *doc = Value::Object(Map::new());
        doc.as_object_mut().expect("just set this to an object")
    };

    for (key, value) in patch_map {
        if value.is_null() {
            map.remove(key);
        } else {
            merge_json_rfc_7396(map.entry(key).or_insert(Value::Null), value);
        }
    }
}
