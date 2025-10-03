use std::{collections::HashSet, fmt::Display, path::Path, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) mod utils;

pub mod configs;
pub mod parse;
pub mod render;

#[derive(Serialize, Deserialize)]
pub struct AcquiesceConfig {
    version: Version,
    config: Acquiesce,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Version {
    V1,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Acquiesce {
    Components {
        allowed_roles: DistinctLiterals,
        tool_calls: Option<ToolCalls>,
    },
    Harmony,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ToolCalls {
    ToolCall {
        tool_call: ToolCall,
    },
    ToolCallsSection {
        prefix: OrderedLiterals,
        tool_call: ToolCall,
        suffix: Option<OrderedLiterals>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ToolCall {
    JsonObject {
        name_key: DistinctLiterals,
        argument_key: DistinctLiterals,
    },
    JsonArray {
        name_key: DistinctLiterals,
        argument_key: DistinctLiterals,
    },
    NamedParameters {
        prefix: Option<OrderedLiterals>,
        delimiter: Option<OrderedLiterals>,
        arguments: Arguments,
        suffix: Option<OrderedLiterals>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arguments {
    JsonObject,
}

impl FromStr for Acquiesce {
    type Err = InitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(serde_json::from_str::<AcquiesceConfig>(s)?.config)
    }
}

impl Display for Acquiesce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let config = AcquiesceConfig {
            version: Version::V1,
            config: self.clone(),
        };

        let json_string = serde_json::to_string_pretty(&config).map_err(|_| std::fmt::Error)?;

        write!(f, "{json_string}")
    }
}

impl Acquiesce {
    pub fn from_file(path: &Path) -> Result<Self, InitError> {
        FromStr::from_str(&std::fs::read_to_string(path)?)
    }

    pub fn to_file(&self, path: &Path) -> Result<(), InitError> {
        std::fs::write(path, self.to_string())?;
        Ok(())
    }
}

pub const DEFAULT_ROLES: &[&str] = &["user", "assistant", "system", "developer", "tool"];

pub fn default_roles() -> DistinctLiterals {
    DistinctLiterals(DEFAULT_ROLES.iter().map(|s| s.to_string()).collect())
}

pub fn default_name_key() -> DistinctLiterals {
    DistinctLiterals(HashSet::from(["name".to_string()]))
}

pub fn default_argument_keys() -> DistinctLiterals {
    DistinctLiterals(HashSet::from([
        "arguments".to_string(),
        "parameters".to_string(),
    ]))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "DistinctLiteralsRepr", into = "DistinctLiteralsRepr")]
pub struct DistinctLiterals(HashSet<String>);

impl From<&str> for DistinctLiterals {
    fn from(s: &str) -> Self {
        DistinctLiterals(HashSet::from([s.to_string()]))
    }
}

impl From<&[&str]> for DistinctLiterals {
    fn from(arr: &[&str]) -> Self {
        DistinctLiterals(arr.iter().map(|s| s.to_string()).collect())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum DistinctLiteralsRepr {
    String(String),
    Array(Vec<String>),
}

impl From<DistinctLiteralsRepr> for DistinctLiterals {
    fn from(repr: DistinctLiteralsRepr) -> Self {
        match repr {
            DistinctLiteralsRepr::String(s) => DistinctLiterals(HashSet::from([s])),
            DistinctLiteralsRepr::Array(arr) => DistinctLiterals(arr.into_iter().collect()),
        }
    }
}

impl From<DistinctLiterals> for DistinctLiteralsRepr {
    fn from(distinct_literals: DistinctLiterals) -> Self {
        let DistinctLiterals(set) = distinct_literals;

        match set.len() {
            1 => DistinctLiteralsRepr::String(set.into_iter().next().unwrap()),
            _ => DistinctLiteralsRepr::Array(set.into_iter().collect()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LiteralOrWild {
    Literal(String),
    Wild {
        wild: WildType,
        #[serde(skip_serializing_if = "Option::is_none")]
        bounded: Option<usize>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WildType {
    Numeric,
    Any,
}

impl From<&str> for LiteralOrWild {
    fn from(s: &str) -> Self {
        LiteralOrWild::Literal(s.to_string())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "OrderedLiteralsRepr", into = "OrderedLiteralsRepr")]
pub struct OrderedLiterals(Vec<LiteralOrWild>);

impl<T: Into<LiteralOrWild>> From<T> for OrderedLiterals {
    fn from(s: T) -> Self {
        OrderedLiterals(vec![s.into()])
    }
}

impl<T: Into<LiteralOrWild> + Clone> From<&[T]> for OrderedLiterals {
    fn from(arr: &[T]) -> Self {
        OrderedLiterals(arr.iter().map(|s| s.clone().into()).collect())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum OrderedLiteralsRepr {
    String(LiteralOrWild),
    Array(Vec<LiteralOrWild>),
}

impl From<OrderedLiteralsRepr> for OrderedLiterals {
    fn from(repr: OrderedLiteralsRepr) -> Self {
        match repr {
            OrderedLiteralsRepr::String(s) => Self(vec![s]),
            OrderedLiteralsRepr::Array(arr) => Self(arr),
        }
    }
}

impl From<OrderedLiterals> for OrderedLiteralsRepr {
    fn from(ordered_literals: OrderedLiterals) -> Self {
        let OrderedLiterals(arr) = ordered_literals;

        match arr.len() {
            1 => OrderedLiteralsRepr::String(arr.into_iter().next().unwrap()),
            _ => OrderedLiteralsRepr::Array(arr.into_iter().collect()),
        }
    }
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("invalid config: {0}")]
    InvalidConfig(#[from] serde_json::Error),

    #[error("config not found: {0}")]
    ConfigNotFound(#[from] std::io::Error),
}
