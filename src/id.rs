use core::fmt;

use serde_json::Value;
use uuid::Uuid;

/// TODO: is this the right name for this data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceId {
    Int,
    Uuidv4,
    Uuidv7,
}

impl ResourceId {
    #[must_use]
    pub fn next_id(self, collection: &[Value]) -> Value {
        match self {
            Self::Uuidv4 => Value::String(Uuid::new_v4().to_string()),
            Self::Uuidv7 => Value::String(Uuid::now_v7().to_string()),
            Self::Int => {
                let max = collection
                    .iter()
                    .filter_map(|item| match *item.get("id")? {
                        Value::String(ref v) => v.parse().ok(),
                        Value::Number(ref v) => v.as_u64(),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                Value::String((max + 1).to_string())
            }
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match *self {
            Self::Uuidv4 | Self::Uuidv7 => "uuid",
            Self::Int => "int",
        }
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}
