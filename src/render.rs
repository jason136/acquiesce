use core::fmt;
use std::{collections::HashMap, fmt::Display, sync::OnceLock};

use llguidance::{ParserFactory, api::TopLevelGrammar, toktrie::ApproximateTokEnv};
use serde_json::json;

use crate::{
    Acquiesce, Arguments, Config, Error, Lexeme, OrderedLexemes, Thinking, ToolCall, ToolCalls,
    render::{
        gbnf::{gbnf_json_schema, gbnf_regex, gbnf_string_literal},
        lark::{lark_json_schema, lark_regex, lark_string_literal, lark_token_literal},
        schema::{
            ChatTool, ChatToolChoice, CustomTool, CustomToolFormat, CustomToolGrammar,
            CustomToolSyntax, FunctionName, FunctionTool,
        },
        template::{TemplateChatMessage, TemplateTool},
    },
};

pub(crate) mod gbnf;
pub(crate) mod lark;

pub mod schema;
pub mod template;

pub enum GrammarSyntax {
    Lark,
    GBNF,
}

pub struct RenderResult {
    pub prompt: String,
    pub grammar: Option<String>,
    // pub parser: Option<Parser>,
}

impl Acquiesce {
    pub fn render(
        &self,
        messages: impl Into<Vec<TemplateChatMessage>>,
        tools: Vec<ChatTool>,
        tool_choice: ChatToolChoice,
        parallel_tool_calls: bool,
        mixed_content_tool_calls: bool,
        grammar_syntax: GrammarSyntax,
    ) -> Result<RenderResult, RenderError> {
        match self {
            Config::Components {
                chat_template,
                thinking,
                tool_calls,
            } => {
                let (Some(tool_calls), false, false) = (
                    tool_calls,
                    tools.is_empty(),
                    matches!(tool_choice, ChatToolChoice::None),
                ) else {
                    let prompt = chat_template.render(messages.into(), &[])?;

                    return Ok(RenderResult {
                        prompt,
                        grammar: None,
                        // parser: None,
                    });
                };

                let validated_tools =
                    tools
                        .into_iter()
                        .try_fold(Vec::new(), |mut tool_acc, tool| {
                            match &tool {
                                ChatTool::Function {
                                    function:
                                        FunctionTool {
                                            name, parameters, ..
                                        },
                                } => {
                                    jsonschema::meta::validate(parameters).map_err(|e| {
                                        RenderError::JsonSchema(name.clone(), e.to_string())
                                    })?;
                                }
                                ChatTool::Custom {
                                    custom: CustomTool { name, format, .. },
                                } => match format {
                                    CustomToolFormat::Text => {}
                                    CustomToolFormat::Grammar {
                                        grammar: CustomToolGrammar { definition, syntax },
                                    } => match syntax {
                                        CustomToolSyntax::Lark => {
                                            static PARSER_FACTORY: OnceLock<ParserFactory> =
                                                OnceLock::new();

                                            let parser_factory = PARSER_FACTORY.get_or_init(|| {
                                                let tok_env = ApproximateTokEnv::single_byte_env();
                                                ParserFactory::new_simple(&tok_env).unwrap()
                                            });

                                            let grammar =
                                                TopLevelGrammar::from_lark(definition.clone());
                                            parser_factory.create_parser(grammar).map_err(|e| {
                                                RenderError::Lark(name.clone(), e.to_string())
                                            })?;
                                        }
                                        CustomToolSyntax::Regex => {
                                            regex::Regex::new(definition).map_err(|e| {
                                                RenderError::Regex(name.clone(), e.to_string())
                                            })?;
                                        }
                                    },
                                },
                            }

                            tool_acc.push(tool.into());

                            Ok::<_, RenderError>(tool_acc)
                        })?;

                let prompt = chat_template.render(messages.into(), &validated_tools)?;

                let mut rules = Rules::new(grammar_syntax);

                let Some((tools_rule, allow_content)) = (match tool_calls {
                    ToolCalls::ToolCall { tool_call } => {
                        tool_choice.render(tool_call, &validated_tools, &mut rules)?
                    }
                    ToolCalls::ToolCallsSection {
                        prefix,
                        tool_call,
                        suffix,
                    } => tool_choice
                        .render(tool_call, &validated_tools, &mut rules)?
                        .map(|(mut tool_choice, allow_content)| {
                            let mut acc = vec![prefix.render(&mut rules)?];

                            if parallel_tool_calls {
                                tool_choice =
                                    rules.insert_repetition("tool_choice", tool_choice, 0, None);
                            }

                            acc.push(tool_choice);

                            if let Some(suffix) = suffix {
                                acc.push(suffix.render(&mut rules)?);
                            }

                            let tools_rule = rules.insert_sequence("tool_choices", &acc);
                            Ok::<_, RenderError>((tools_rule, allow_content))
                        })
                        .transpose()?,
                }) else {
                    return Ok(RenderResult {
                        prompt,
                        grammar: None,
                        // parser: None,
                    });
                };

                let text_rule = rules.insert_text_lexeme()?;
                let mut acc = Vec::new();

                if let Some(Thinking { prefix, suffix }) = thinking {
                    acc.push(prefix.render(&mut rules)?);
                    acc.push(text_rule.clone());
                    acc.push(suffix.render(&mut rules)?);
                }

                if allow_content || mixed_content_tool_calls {
                    acc.push(text_rule.clone());
                }

                acc.push(tools_rule);

                let root = rules.insert_sequence("root", &acc);
                let grammar = rules.resolve(root);

                Ok(RenderResult {
                    prompt,
                    grammar: Some(grammar),
                    // parser: self.parser(),
                })
            }
            Config::Harmony => Ok(RenderResult {
                prompt: String::new(),
                grammar: None,
                // parser: None,
            }),
        }
    }
}

impl OrderedLexemes {
    fn render(&self, rules: &mut Rules) -> Result<RuleKey, RenderError> {
        let OrderedLexemes(literals) = self;

        let sequence_keys = literals
            .iter()
            .map(|lexeme| rules.insert_lexeme("sequence", lexeme))
            .collect::<Result<Vec<_>, RenderError>>()?;

        Ok(rules.insert_sequence("sequence", &sequence_keys))
    }
}

impl TemplateTool {
    fn naive_json_schema(&self, name_key: &str, argument_key: &str) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "enum": [
                        name_key,
                    ],
                },
                argument_key: {
                    "type": "object",
                    "properties": self.parameters,
                },
                "required": [
                    "name",
                    argument_key,
                ]
            }
        })
    }
}

impl ChatToolChoice {
    fn render(
        &self,
        tool_call: &ToolCall,
        validated_tools: &[TemplateTool],
        rules: &mut Rules,
    ) -> Result<Option<(RuleKey, bool)>, RenderError> {
        Ok(match self {
            ChatToolChoice::Auto => {
                let tool_choice = tool_call.render(validated_tools, rules)?;
                let tool_choice = rules.insert_repetition("tool_choice", tool_choice, 0, Some(1));

                Some((tool_choice, true))
            }
            ChatToolChoice::None => None,
            ChatToolChoice::Required => Some((tool_call.render(validated_tools, rules)?, false)),
            ChatToolChoice::Function(FunctionName { name }) => {
                let selected_tool = validated_tools
                    .iter()
                    .find(|tool| &tool.name == name)
                    .ok_or(RenderError::ChatToolChoice)?;
                let tool_choice = tool_call.render(std::slice::from_ref(selected_tool), rules)?;

                Some((tool_choice, false))
            }
        })
    }
}

impl ToolCall {
    fn render(&self, tools: &[TemplateTool], rules: &mut Rules) -> Result<RuleKey, RenderError> {
        match self {
            ToolCall::JsonObject {
                name_key,
                argument_key,
            } => {
                let schema_choices = tools
                    .iter()
                    .map(|tool| tool.naive_json_schema(name_key, argument_key))
                    .collect::<Vec<_>>();

                let object_schema = json!({
                    "anyOf": schema_choices,
                });

                Ok(rules.insert_lexeme("tool_choice", &Lexeme::JsonSchema(object_schema))?)
            }
            ToolCall::JsonArray {
                name_key,
                argument_key,
            } => {
                let schema_choices = tools
                    .iter()
                    .map(|tool| tool.naive_json_schema(name_key, argument_key))
                    .collect::<Vec<_>>();

                let array_schema = json!({
                    "type": "array",
                    "items": {
                        "anyOf": schema_choices,
                    },
                });

                Ok(rules.insert_lexeme("tool_choice", &Lexeme::JsonSchema(array_schema))?)
            }
            ToolCall::NamedParameters {
                prefix,
                delimiter,
                arguments,
                suffix,
            } => {
                let alternative_keys = tools
                    .iter()
                    .map(|tool| {
                        let mut acc = Vec::new();

                        if let Some(prefix) = prefix {
                            acc.push(prefix.render(rules)?);
                        }

                        acc.push(rules.insert_lexeme("name", &Lexeme::Text(tool.name.clone()))?);

                        if let Some(delimiter) = delimiter {
                            acc.push(delimiter.render(rules)?);
                        }

                        match arguments {
                            Arguments::JsonObject => {
                                acc.push(rules.insert_lexeme(
                                    "parameters",
                                    &Lexeme::JsonSchema(tool.parameters.clone()),
                                )?);
                            }
                        }

                        if let Some(suffix) = suffix {
                            acc.push(suffix.render(rules)?);
                        }

                        Ok(rules.insert_sequence("tool_choice_item", &acc))
                    })
                    .collect::<Result<Vec<_>, RenderError>>()?;

                Ok(rules.insert_alternative("tool_choices", &alternative_keys))
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct RuleKey(String, usize);

impl Display for RuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.0, self.1)
    }
}

struct Rules {
    rules: HashMap<RuleKey, String>,
    syntax: GrammarSyntax,
}

impl Rules {
    fn new(syntax: GrammarSyntax) -> Self {
        Self {
            rules: HashMap::new(),
            syntax,
        }
    }

    fn insert_sequence(&mut self, key: &str, sequence_keys: &[RuleKey]) -> RuleKey {
        let rule = sequence_keys
            .iter()
            .map(|rule_key| rule_key.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        self.insert_rule(key, rule)
    }

    fn insert_alternative(&mut self, key: &str, alternative_keys: &[RuleKey]) -> RuleKey {
        let rule = alternative_keys
            .iter()
            .map(|rule_key| rule_key.to_string())
            .collect::<Vec<_>>()
            .join(" | ");

        self.insert_rule(key, rule)
    }

    fn insert_repetition(
        &mut self,
        key: &str,
        repetition_key: RuleKey,
        start: usize,
        end: Option<usize>,
    ) -> RuleKey {
        let rule = match (start, end) {
            (0, None) => format!("{}*", repetition_key),
            (1, None) => format!("{}+", repetition_key),
            (0, Some(1)) => format!("{}?", repetition_key),
            (exact, Some(maybe_exact)) if exact == maybe_exact => {
                format!("{}{{{}}}", repetition_key, exact)
            }
            (at_least, None) => format!("{}{{{},}}", repetition_key, at_least),
            (at_least, Some(at_most)) => format!("{}{{{},{}}}", repetition_key, at_least, at_most),
        };

        self.insert_rule(key, rule)
    }

    fn insert_lexeme(&mut self, key: &str, lexeme: &Lexeme) -> Result<RuleKey, RenderError> {
        match self.syntax {
            GrammarSyntax::Lark => {
                let rule = match lexeme {
                    Lexeme::Text(text) => lark_string_literal(text),
                    Lexeme::Token(token) => lark_token_literal(token),
                    Lexeme::Regex { pattern } => lark_regex(pattern),
                    Lexeme::JsonSchema(json_schema) => lark_json_schema(json_schema),
                };

                Ok(self.insert_rule(&key.to_uppercase(), rule))
            }
            GrammarSyntax::GBNF => {
                let rule = match lexeme {
                    Lexeme::Text(text) => gbnf_string_literal(text),
                    Lexeme::Token(token) => gbnf_string_literal(token),
                    Lexeme::Regex { pattern } => gbnf_regex(pattern),
                    Lexeme::JsonSchema(json_schema) => gbnf_json_schema(json_schema)?,
                };

                Ok(self.insert_rule(key, rule))
            }
        }
    }

    fn insert_rule(&mut self, key: &str, value: String) -> RuleKey {
        let mut count = 0;
        let mut rule_key = RuleKey(key.to_string(), count);

        while let Some(rule) = self.rules.get(&rule_key) {
            if rule == &value {
                return rule_key;
            }

            count += 1;
            rule_key.1 = count;
        }

        self.rules.insert(rule_key.clone(), value);

        rule_key
    }

    fn insert_text_lexeme(&mut self) -> Result<RuleKey, RenderError> {
        match self.syntax {
            GrammarSyntax::Lark => {
                self.insert_lexeme("text", &Lexeme::Text(lark::TEXT.to_string()))
            }
            GrammarSyntax::GBNF => {
                self.insert_lexeme("text", &Lexeme::Text(gbnf::TEXT.to_string()))
            }
        }
    }

    fn resolve(&mut self, root_key: RuleKey) -> String {
        let root = self.rules.remove(&root_key).unwrap_or_default();

        match self.syntax {
            GrammarSyntax::Lark => {
                format!(
                    "start: {root}\n{}",
                    self.rules
                        .iter()
                        .map(|(key, value)| format!("{key}: {value}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            GrammarSyntax::GBNF => {
                format!(
                    "root ::= {root}\n{}",
                    self.rules
                        .iter()
                        .map(|(key, value)| format!("{key} ::= {value}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("json schema for tool {0} is invalid: {1}")]
    JsonSchema(String, String),

    #[error("regex for tool {0} is invalid: {1}")]
    Regex(String, String),

    #[error("tool choice not found in provided tools")]
    ChatToolChoice,

    #[error("lark grammar for tool {0} is invalid: {1}")]
    Lark(String, String),

    #[error("python error: {0}")]
    Python(#[from] pyo3::PyErr),

    #[error("chat template render error: {0}")]
    Template(#[from] minijinja::Error),

    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
