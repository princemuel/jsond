//! Thread-safe in-memory JSON database with atomic file persistence.
//!
//! Top-level keys are resource names.
//! - A Value array is a collection resource (GET / POST / PUT / PATCH / DELETE)
//! - A Value object is a singleton resource (GET / PUT / PATCH)
//!
//! Each created item gets an auto-generated ID whose format depends on the
//! resource id strategy is chosen at startup (uuidv4, uuidv7, or int).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::error::{Error, Result};
use crate::id::IdStrategy;

#[derive(Clone)]
pub struct Database(Arc<RwLock<Inner>>);

#[derive(Clone)]
pub struct Inner {
    pub data: Map<String, Value>,
    pub path: PathBuf,
    pub id_strategy: IdStrategy,
    pub readonly: bool,
}

impl Database {
    /// Load a database from a JSON or JSON5 file.
    pub fn load(path: impl AsRef<Path>, id_strategy: IdStrategy, readonly: bool) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let content = fs::read_to_string(&path)?;
        let data = parse_db(&content, &path)?;
        Ok(Self(Arc::new(RwLock::new(Inner { data, path, id_strategy, readonly }))))
    }

    /// Reload from disk (used by file watcher).
    pub async fn reload(&self) -> Result<()> {
        let mut g = self.write().await;
        let content = fs::read_to_string(&g.path)?;

        g.data = parse_db(&content, &g.path)?;
        tracing::info!("Reloaded database from {}", g.path.display());
        Ok(())
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, Inner> { self.0.read().await }

    pub async fn write(&self) -> RwLockWriteGuard<'_, Inner> { self.0.write().await }

    // ##### Introspection #####

    /// Get the names of all top-level keys.
    pub async fn resources(&self) -> Vec<String> { self.read().await.data.keys().cloned().collect() }

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
        self.0.read().await.data.get(resource).and_then(|v| v.as_array()).cloned()
    }

    /// Get a singleton (object).
    pub async fn get_singleton(&self, resource: &str) -> Option<Value> {
        self.0.read().await.data.get(resource).filter(|v| v.is_object()).cloned()
    }

    /// Find a single item by its `id` field.
    pub async fn find(&self, resource: &str, id: &str) -> Option<Value> {
        self.get_collection(resource).await?.into_iter().find(|item| id_matches(item, id))
    }

    /// Insert a new item, assigning a string id if one is not present.
    pub async fn insert(&self, resource: &str, mut item: Value) -> Result<Value> {
        let mut g = self.write().await;

        if item.get("id").is_none() {
            // let collection = g
            //     .data
            //     .get(resource)
            //     .and_then(Value::as_array)
            //     .map(Vec::as_slice)
            //     .unwrap_or(&[]);

            const EMPTY: &[Value] = &[];
            let collection =
                g.data.get(resource).and_then(Value::as_array).map_or_else(|| EMPTY, Vec::as_slice);

            let id = g.id_strategy.generate(collection);
            normalize_id(&mut item);

            item.as_object_mut()
                .ok_or_else(|| Error::BadRequest("body must be a JSON object".to_owned()))?
                .insert("id".to_owned(), Value::String(id));
        }

        match g.data.get_mut(resource) {
            Some(&mut Value::Array(ref mut v)) => v.push(item.clone()),
            Some(_) => return Err(Error::NotCollection(resource.to_owned())),
            None => {
                g.data.insert(resource.to_owned(), Value::Array(vec![item.clone()]));
            }
        }

        persist(&g)?;
        Ok(item)
    }

    /// Full replace (PUT). Uses the id from the url in the body.
    pub async fn replace(&self, resource: &str, id: &str, mut item: Value) -> Result<Value> {
        item.as_object_mut()
            .ok_or_else(|| Error::BadRequest("body must be a JSON object".to_owned()))?
            .insert("id".to_owned(), Value::String(id.to_owned()));

        let mut g = self.write().await;
        let arr = collection_mut(&mut g, resource)?;
        let pos = find_pos(arr, id).ok_or(Error::NotFound)?;

        if let Some(slot) = arr.get_mut(pos) {
            *slot = item.clone();
        } else {
            return Err(Error::NotFound);
        }

        persist(&g)?;
        Ok(item)
    }

    /// Partial update (PATCH). Merges; the `id` is immutable.
    pub async fn patch(&self, resource: &str, id: &str, item: Value) -> Result<Value> {
        let payload = item
            .as_object()
            .ok_or_else(|| Error::BadRequest("body must be a JSON object".to_owned()))?;

        let mut g = self.write().await;
        let arr = collection_mut(&mut g, resource)?;
        let pos = find_pos(arr, id).ok_or(Error::NotFound)?;

        // Verify the element is an object before mutating
        arr.get(pos)
            .and_then(|v| v.as_object())
            .ok_or_else(|| Error::NotCollection(resource.to_owned()))?;

        let existing = arr.get_mut(pos).ok_or(Error::NotFound)?;

        // Build merge patch without the `id` field
        let mut patch_value = Value::Object(payload.clone());
        patch_value.as_object_mut().ok_or(Error::NotFound)?.remove("id");

        // RFC 7396 JSON Merge Patch: nulls delete keys, objects recurse, scalars
        // replace
        json_patch::merge(existing, &patch_value);

        let item = arr.get(pos).cloned().ok_or(Error::NotFound)?;
        persist(&g)?;
        Ok(item)
    }

    /// Delete an item. also delete dependents if `dependent_resource` is given
    pub async fn delete(&self, resource: &str, id: &str, dependent: Option<&str>) -> Result<Value> {
        let mut g = self.write().await;
        let arr = collection_mut(&mut g, resource)?;
        let pos = find_pos(arr, id).ok_or(Error::NotFound)?;
        let item = arr.remove(pos);

        // Cascading: remove all items in `dependent` where `<resource_singular>Id ==
        // id`
        if let Some(key) = dependent {
            let fk = format!("{}Id", singular(resource));

            if let Some(&mut Value::Array(ref mut v)) = g.data.get_mut(key) {
                v.retain(|item| {
                    item.get(&fk).and_then(Value::as_str) != Some(id)
                        && item.get(&fk).map(|v| v.to_string().trim_matches('"').to_owned()).as_deref()
                            != Some(id)
                });
            }
        }

        persist(&g)?;
        Ok(item)
    }

    /// Replace a singleton entirely (PUT).
    pub async fn replace_singleton(&self, resource: &str, item: Value) -> Result<Value> {
        let mut g = self.write().await;
        if !matches!(g.data.get(resource), Some(Value::Object(_))) {
            return Err(Error::NotFound);
        }
        g.data.insert(resource.to_owned(), item.clone());

        persist(&g)?;
        Ok(item)
    }

    /// Merge-patch a singleton (PATCH).
    pub async fn patch_singleton(&self, resource: &str, patch: Value) -> Result<Value> {
        let mut g = self.write().await;
        let Some(&mut Value::Object(ref mut payload)) = g.data.get_mut(resource) else {
            return Err(Error::NotFound);
        };

        if let Value::Object(p) = patch {
            payload.extend(p);
        }

        let item = Value::Object(payload.clone());
        persist(&g)?;
        Ok(item)
    }
}

fn parse_db(raw: &str, path: &Path) -> Result<Map<String, Value>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let value: Value = if ext == "json5" {
        json5::from_str(raw).map_err(|e| Error::BadRequest(e.to_string()))?
    } else {
        serde_json::from_str(raw)
            .or_else(|_e| json5::from_str(raw))
            .map_err(|e| Error::BadRequest(e.to_string()))?
    };

    value.as_object().cloned().ok_or_else(|| {
        Error::BadRequest("top-level JSON must be an object, e.g { \"posts\": [...] }".into())
    })
}

/// Ensure the `id` field is stored as a string
pub fn normalize_id(item: &mut Value) {
    if let Some(obj) = item.as_object_mut()
        && let Some(id) = obj.get("id").cloned()
    {
        let s = match id {
            Value::String(v) => v,
            v => v.to_string(),
        };

        obj.insert("id".to_owned(), Value::String(s));
    }
}

/// Compare an id value (which may be Number or String) against a string.
#[must_use]
#[expect(clippy::pattern_type_mismatch)]
fn id_matches(item: &Value, id: &str) -> bool {
    match item.get("id") {
        Some(Value::String(v)) => v == id,
        Some(v) => v.to_string().trim_matches('"') == id,
        None => false,
    }
}

fn collection_mut<'a>(g: &'a mut Inner, resource: &'a str) -> Result<&'a mut Vec<Value>> {
    match g.data.get_mut(resource) {
        Some(&mut Value::Array(ref mut v)) => Ok(v),
        Some(_) => Err(Error::NotCollection(resource.to_owned())),
        None => Err(Error::NotFound),
    }
}

fn find_pos(arr: &[Value], id: &str) -> Option<usize> {
    arr.iter().position(|item| id_matches(item, id))
}

fn persist(g: &Inner) -> Result<()> {
    if !g.readonly {
        let tmp = g.path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(&g.data)?;

        fs::write(&tmp, json)?;
        fs::rename(&tmp, &g.path)?;

        tracing::debug!("Persisted database to {}", g.path.display());
    }

    Ok(())
}

/// naive singularizer: strips trailing `s`.
/// `posts` -> `post`, `comments` -> `comment`
fn singular(s: &str) -> &str { s.strip_suffix("s").unwrap_or(s) }
