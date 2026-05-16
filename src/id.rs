use core::fmt;

use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum IdStrategy {
    Int,
    Uuidv4,
    Uuidv7,
}

impl IdStrategy {
    #[must_use]
    pub fn generate(self, collection: &[Value]) -> String {
        match self {
            Self::Uuidv4 => Uuid::new_v4().to_string(),
            Self::Uuidv7 => Uuid::now_v7().to_string(),
            // scan existing items and return max+1 or fallback to "1"
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
                (max + 1).to_string()
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

impl fmt::Display for IdStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}
