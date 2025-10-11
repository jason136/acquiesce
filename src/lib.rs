use std::{collections::HashSet, fmt::Display, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::render::template::ChatTemplate;

pub(crate) mod utils;

pub mod configs;
pub mod parse;
pub mod render;

pub static ACQUIESCE_CONFIG: &str = "acquiesce.json";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Version {
    V1,
}

#[derive(Serialize, Deserialize)]
pub struct AcquiesceConfig {
    version: Version,
    config: AcquiesceRepr,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arguments {
    JsonObject,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ToolCall {
    JsonObject {
        name_key: String,
        argument_key: String,
    },
    JsonArray {
        name_key: String,
        argument_key: String,
    },
    NamedParameters {
        prefix: Option<OrderedLiterals>,
        delimiter: Option<OrderedLiterals>,
        arguments: Arguments,
        suffix: Option<OrderedLiterals>,
    },
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
pub struct Thinking {
    prefix: String,
    suffix: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Config<T> {
    Components {
        chat_template: T,
        thinking: Option<Thinking>,
        tool_calls: Option<ToolCalls>,
    },
    Harmony,
}

pub type AcquiesceRepr = Config<()>;

pub type Acquiesce = Config<ChatTemplate>;

impl Display for AcquiesceRepr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let config = AcquiesceConfig {
            version: Version::V1,
            config: self.clone(),
        };

        let json_string = serde_json::to_string_pretty(&config).map_err(|_| std::fmt::Error)?;

        write!(f, "{json_string}")
    }
}

impl TryFrom<(AcquiesceRepr, &Path)> for Acquiesce {
    type Error = InitError;

    fn try_from(value: (AcquiesceRepr, &Path)) -> Result<Self, Self::Error> {
        let (repr, dir) = value;

        let acquiesce = match repr {
            Config::Components {
                tool_calls,
                thinking,
                ..
            } => Acquiesce::Components {
                chat_template: ChatTemplate::from_repo(dir)?,
                thinking,
                tool_calls,
            },
            Config::Harmony => Config::Harmony,
        };

        Ok(acquiesce)
    }
}

impl Acquiesce {
    pub fn from_repo(dir: &Path) -> Result<Self, InitError> {
        if !dir.is_dir() {
            return Err(InitError::InvalidDir);
        }

        let config_string = std::fs::read_to_string(dir.join(ACQUIESCE_CONFIG))?;

        let repr = serde_json::from_str::<AcquiesceConfig>(&config_string)?.config;

        Self::try_from((repr, dir))
    }

    pub fn from_repo_with_fallback(dir: &Path, fallback: AcquiesceRepr) -> Result<Self, InitError> {
        if !dir.is_dir() {
            return Err(InitError::InvalidDir);
        }

        let repr = if let Ok(repr) = std::fs::read_to_string(dir.join(ACQUIESCE_CONFIG)) {
            serde_json::from_str::<AcquiesceConfig>(&repr)?.config
        } else {
            fallback
        };

        Self::try_from((repr, dir))
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

    #[error("path must be a directory")]
    InvalidDir,

    #[error("config not found: {0}")]
    ConfigNotFound(#[from] std::io::Error),

    #[error("chat template not found")]
    MissingTemplate,

    #[error("chat template compilation error: {0}")]
    TemplateCompilation(#[from] minijinja::Error),
}
