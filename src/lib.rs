use std::{collections::HashSet, fmt::Display};

use hf_hub::CacheRepo;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{configs::kimik2::kimi_k2, render::template::ChatTemplate};

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
        prefix: Option<OrderedLexemes>,
        delimiter: Option<OrderedLexemes>,
        arguments: Arguments,
        suffix: Option<OrderedLexemes>,
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
        prefix: OrderedLexemes,
        tool_call: ToolCall,
        suffix: Option<OrderedLexemes>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Thinking {
    prefix: OrderedLexemes,
    suffix: OrderedLexemes,
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

impl Acquiesce {
    pub fn from_repo(repo: &CacheRepo) -> Result<Self, InitError> {
        let config_string = std::fs::read_to_string(
            repo.get(ACQUIESCE_CONFIG)
                .ok_or(InitError::ConfigNotFound(ACQUIESCE_CONFIG))?,
        )?;

        let repr = serde_json::from_str::<AcquiesceConfig>(&config_string)?.config;

        repr.resolve_from_repo(repo)
    }
}

impl AcquiesceRepr {
    pub fn resolve_from_repo(self, repo: &CacheRepo) -> Result<Acquiesce, InitError> {
        Ok(match self {
            Config::Components {
                tool_calls,
                thinking,
                ..
            } => Acquiesce::Components {
                chat_template: ChatTemplate::from_repo(repo)?,
                thinking,
                tool_calls,
            },
            Config::Harmony => Config::Harmony,
        })
    }

    pub fn resolve_from_options(
        self,
        chat_template: String,
        bos_token: Option<String>,
        eos_token: Option<String>,
        multimodal: bool,
        add_generation_prompt: bool,
    ) -> Result<Acquiesce, InitError> {
        Ok(match self {
            Config::Components {
                thinking,
                tool_calls,
                ..
            } => Acquiesce::Components {
                chat_template: ChatTemplate::from_options(
                    chat_template,
                    bos_token,
                    eos_token,
                    multimodal,
                    add_generation_prompt,
                )?,
                thinking,
                tool_calls,
            },
            Config::Harmony => Config::Harmony,
        })
    }

    pub fn infer_default(model_name: &str) -> Result<Self, InitError> {
        let model = model_name.trim().to_lowercase();

        match model {
            _ if ["kimi", "k2"].iter().all(|m| model.contains(m)) => Ok(kimi_k2()),
            _ => Err(InitError::InferFailed),
        }
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
pub enum Lexeme {
    Text(String),
    Token(String),
    Regex { pattern: String },
    JsonSchema(serde_json::Value),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "OrderedLexemesRepr", into = "OrderedLexemesRepr")]
pub struct OrderedLexemes(Vec<Lexeme>);

impl<T: Into<Lexeme>> From<T> for OrderedLexemes {
    fn from(s: T) -> Self {
        OrderedLexemes(vec![s.into()])
    }
}

impl<T: Into<Lexeme> + Clone> From<&[T]> for OrderedLexemes {
    fn from(arr: &[T]) -> Self {
        OrderedLexemes(arr.iter().map(|s| s.clone().into()).collect())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum OrderedLexemesRepr {
    String(Lexeme),
    Array(Vec<Lexeme>),
}

impl From<OrderedLexemesRepr> for OrderedLexemes {
    fn from(repr: OrderedLexemesRepr) -> Self {
        match repr {
            OrderedLexemesRepr::String(s) => Self(vec![s]),
            OrderedLexemesRepr::Array(arr) => Self(arr),
        }
    }
}

impl From<OrderedLexemes> for OrderedLexemesRepr {
    fn from(ordered_lexemes: OrderedLexemes) -> Self {
        let OrderedLexemes(arr) = ordered_lexemes;

        match arr.len() {
            1 => OrderedLexemesRepr::String(arr.into_iter().next().unwrap()),
            _ => OrderedLexemesRepr::Array(arr.into_iter().collect()),
        }
    }
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("invalid config: {0}")]
    InvalidConfig(#[from] serde_json::Error),

    #[error("failed to read config: {0}")]
    FailedToReadConfig(#[from] std::io::Error),

    #[error("required config not found: {0}")]
    ConfigNotFound(&'static str),

    #[error("failed to infer default config")]
    InferFailed,

    #[error("chat template not found")]
    MissingTemplate,

    #[error("chat template compilation error: {0}")]
    TemplateCompilation(#[from] minijinja::Error),

    #[error("fallback chat template compilation error: {0}")]
    FallbackTemplateCompilation(#[from] pyo3::PyErr),
}
