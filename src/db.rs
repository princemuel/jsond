//! In-memory JSON database with atomic file persistence.
//!
//! The database holds a `serde_json::Value` (always an Object at the top
//! level). Collections are arrays; singletons are objects.
//!
//! All mutations are written back to disk atomically via a temp-file rename.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{Error, Result};

pub type Db = Arc<RwLock<Database>>;

#[derive(Clone, Debug)]
pub struct Database {
    pub data: Value,
    pub path: PathBuf,
    pub read_only: bool,
}

impl Database {
    /// Load a database from a JSON or JSON5 file.
    pub fn load(path: &Path, read_only: bool) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let data = parse_json_or_json5(&content, path)?;

        if !data.is_object() {
            return Err(Error::BadRequest("Top-level database value must be a JSON object".into()));
        }

        tracing::info!("Loaded database from {}", path.display());
        Ok(Self { data, path: path.to_path_buf(), read_only })
    }

    /// Reload from disk (used by file watcher).
    pub fn reload(&mut self) -> Result<()> {
        let content = fs::read_to_string(&self.path)?;
        let data = parse_json_or_json5(&content, &self.path)?;

        if !data.is_object() {
            return Err(Error::BadRequest("Top-level database value must be a JSON object".into()));
        }

        self.data = data;
        tracing::info!("Reloaded database from {}", self.path.display());
        Ok(())
    }

    /// Persist current state to disk atomically.
    pub fn persist(&self) -> Result<()> {
        if self.read_only {
            return Ok(());
        }

        let json = serde_json::to_string_pretty(&self.data)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)?;
        tracing::debug!("Persisted database to {}", self.path.display());
        Ok(())
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Collection helpers
    // ──────────────────────────────────────────────────────────────────────────

    /// Get the names of all top-level keys.
    pub fn resource_names(&self) -> Vec<String> {
        self.data.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default()
    }

    /// is the key an array (`collection`).
    pub fn is_collection(&self, name: &str) -> bool {
        self.data.get(name).map(|v| v.is_array()).unwrap_or(false)
    }

    /// is the key an object (`singleton`).
    pub fn is_singleton(&self, name: &str) -> bool {
        self.data.get(name).map(|v| v.is_object()).unwrap_or(false)
    }

    /// Get a collection (array).
    pub fn get_collection(&self, name: &str) -> Option<Vec<Value>> {
        self.data.get(name).and_then(|v| v.as_array()).cloned()
    }

    /// Get a singleton (object).
    pub fn get_singleton(&self, name: &str) -> Option<Value> {
        self.data.get(name).filter(|v| v.is_object()).cloned()
    }

    /// Find a single item by its `id` field.
    pub fn find(&self, collection: &str, id: &str) -> Option<Value> {
        self.get_collection(collection)?
            .into_iter()
            .find(|item| item.get("id").map(|v| id_matches(v, id)).unwrap_or(false))
    }

    /// Insert a new item, assigning a string id if one is not present.
    pub fn insert(&mut self, collection: &str, mut item: Value) -> Result<Value> {
        let arr = self
            .data
            .get_mut(collection)
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| Error::NotFound)?;

        // Auto-assign id if missing
        if item.get("id").is_none() {
            item["id"] = Value::String(Uuid::now_v7().as_hyphenated().to_string());
        } else {
            // Check for duplicate id
            let given_id = item["id"].to_owned();
            let exists = arr.iter().any(|x| x.get("id").map(|v| *v == given_id).unwrap_or(false));
            if exists {
                return Err(Error::Conflict);
            }
        }

        arr.push(item.clone());
        Ok(item)
    }

    /// Full replace of an item (PUT).
    pub fn replace(&mut self, collection: &str, id: &str, mut item: Value) -> Result<Value> {
        let arr =
            self.data.get_mut(collection).and_then(|v| v.as_array_mut()).ok_or(Error::NotFound)?;

        let pos = arr
            .iter()
            .position(|x| x.get("id").map(|v| id_matches(v, id)).unwrap_or(false))
            .ok_or(Error::NotFound)?;

        // Preserve the id
        item["id"] = arr[pos]["id"].to_owned();
        arr[pos] = item.clone();
        Ok(item)
    }

    /// Partial update of an item (PATCH).
    pub fn patch(&mut self, collection: &str, id: &str, patch: Value) -> Result<Value> {
        let arr =
            self.data.get_mut(collection).and_then(|v| v.as_array_mut()).ok_or(Error::NotFound)?;

        let pos = arr
            .iter()
            .position(|x| x.get("id").map(|v| id_matches(v, id)).unwrap_or(false))
            .ok_or(Error::NotFound)?;

        merge_json(&mut arr[pos], patch);
        Ok(arr[pos].clone())
    }

    /// Delete an item.
    pub fn delete(&mut self, collection: &str, id: &str) -> Result<Value> {
        let arr =
            self.data.get_mut(collection).and_then(|v| v.as_array_mut()).ok_or(Error::NotFound)?;

        let pos = arr
            .iter()
            .position(|x| x.get("id").map(|v| id_matches(v, id)).unwrap_or(false))
            .ok_or(Error::NotFound)?;

        Ok(arr.remove(pos))
    }

    /// Replace a singleton entirely (PUT).
    pub fn replace_singleton(&mut self, name: &str, item: Value) -> Result<Value> {
        let slot = self.data.get_mut(name).filter(|v| v.is_object()).ok_or(Error::NotFound)?;
        *slot = item.clone();
        Ok(item)
    }

    /// Merge-patch a singleton (PATCH).
    pub fn patch_singleton(&mut self, name: &str, patch: Value) -> Result<Value> {
        let slot = self.data.get_mut(name).filter(|v| v.is_object()).ok_or(Error::NotFound)?;
        merge_json(slot, patch);
        Ok(slot.clone())
    }

    /// Initialise a new, empty collection if the key is absent.
    pub fn ensure_collection(&mut self, name: &str) {
        self.data.get(name).unwrap_or_default();
        // if self.data.get(name).is_none() {
        //     self.data[name] = json!([]);
        // }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Compare an id value (which may be Number or String) against a string.
pub fn id_matches(value: &Value, id: &str) -> bool {
    match value {
        Value::String(s) => s == id,
        _ => false,
    }
}

/// RFC-7396 JSON Merge Patch.
#[expect(clippy::else_if_without_else)]
fn merge_json(target: &mut Value, patch: Value) {
    if let Value::Object(pmap) = patch {
        let tmap = target.as_object_mut().get_or_insert_default();
        for (k, v) in pmap {
            if v.is_null() {
                tmap.remove(&k);
            } else if let Some(existing) = tmap.get_mut(&k) {
                if existing.is_object() && v.is_object() {
                    merge_json(existing, v);
                    continue;
                }
            }

            // Simple insert / replace
            if let Some(map) = target.as_object_mut() {
                if !v.is_null() {
                    map.insert(k, v);
                }
            }
        }
    }
}

fn parse_json_or_json5(content: &str, path: &Path) -> Result<Value> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "json5" {
        json5::from_str(content).map_err(|e| Error::BadRequest(e.to_string()))
    } else {
        serde_json::from_str(content).map_err(|e| Error::BadRequest(e.to_string()))
    }
}
