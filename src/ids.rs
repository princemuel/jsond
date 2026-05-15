use core::fmt;

use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdStrategy {
    Uuidv4,
    /// Time-sortable UUID — lexicographically ordered, k-sortable in indexes
    Uuidv7,
    Int, // auto-increment integer, per-collection max+1
}

impl IdStrategy {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Uuidv4 | Self::Uuidv7 => "uuid",
            Self::Int => "int",
        }
    }

    pub fn uuid(&self) -> Option<String> {
        match self {
            IdStrategy::Uuidv4 => Some(Uuid::new_v4().to_string()),
            IdStrategy::Uuidv7 => Some(Uuid::now_v7().to_string()),
            IdStrategy::Int => None,
        }
    }

    /// scans existing items and return max+1.
    /// it falls back to "1" on an empty collection.
    pub fn int(collection: &[Value]) -> String {
        let max = collection
            .iter()
            .filter_map(|item| {
                item.get("id").and_then(|id| match id {
                    Value::String(v) => v.parse().ok(),
                    Value::Number(v) => v.as_u64(),
                    _ => None,
                })
            })
            .max()
            .unwrap_or(0);
        (max + 1).to_string()
    }
}

impl fmt::Display for IdStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
