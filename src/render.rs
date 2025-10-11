use std::{collections::HashSet, sync::OnceLock};

use llguidance::{ParserFactory, api::TopLevelGrammar, toktrie::ApproximateTokEnv};
use serde_json::json;

use crate::{
    Acquiesce, Arguments, Config, Error, LiteralOrWild, OrderedLiterals, Thinking, ToolCall,
    ToolCalls, WildType,
    parse::Parser,
    render::{
        lark::{lark_json_schema, lark_string_literal},
        schema::{
            ChatMessages, ChatTool, ChatToolChoice, CustomTool, CustomToolFormat,
            CustomToolGrammar, CustomToolSyntax, FunctionName, FunctionTool,
        },
        template::TemplateTool,
    },
};

pub(crate) mod lark;
pub(crate) mod template;

pub mod schema;

pub enum GrammarType {
    Lark,
}

pub struct RenderResult {
    pub prompt: String,
    pub grammar: Option<String>,
    pub parser: Option<Parser>,
}

impl Acquiesce {
    pub fn render(
        &self,
        messages: ChatMessages,
        tools: Vec<ChatTool>,
        tool_choice: ChatToolChoice,
        parallel_tool_calls: bool,
        grammar_type: GrammarType,
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
                        parser: None,
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

                let grammar = match grammar_type {
                    GrammarType::Lark => {
                        let mut rules = HashSet::new();
                        rules.insert(lark::TEXT.to_string());

                        fn render_tool_choice(
                            tool_call: &ToolCall,
                            tool_choice: &ChatToolChoice,
                            validated_tools: &[TemplateTool],
                            rules: &mut HashSet<String>,
                        ) -> Result<String, RenderError> {
                            Ok(match &tool_choice {
                                ChatToolChoice::Auto => {
                                    format!("({})?", tool_call.render_lark(validated_tools, rules))
                                }
                                ChatToolChoice::None => String::new(),
                                ChatToolChoice::Required => {
                                    format!("({})", tool_call.render_lark(validated_tools, rules))
                                }
                                ChatToolChoice::Function(FunctionName { name }) => {
                                    let selected_tool = validated_tools
                                        .iter()
                                        .find(|tool| &tool.name == name)
                                        .ok_or(RenderError::ChatToolChoice)?;

                                    format!(
                                        "({})",
                                        tool_call.render_lark(
                                            std::slice::from_ref(selected_tool),
                                            rules
                                        )
                                    )
                                }
                            })
                        }

                        let tool_call = match tool_calls {
                            ToolCalls::ToolCall { tool_call } => render_tool_choice(
                                tool_call,
                                &tool_choice,
                                &validated_tools,
                                &mut rules,
                            )?,
                            ToolCalls::ToolCallsSection {
                                prefix,
                                tool_call,
                                suffix,
                            } => {
                                let mut acc = vec![prefix.render_lark(&mut rules)];

                                let repetition = if parallel_tool_calls { "+" } else { "" };

                                acc.push(format!(
                                    "({}){repetition}",
                                    render_tool_choice(
                                        tool_call,
                                        &tool_choice,
                                        &validated_tools,
                                        &mut rules,
                                    )?
                                ));

                                if let Some(suffix) = suffix {
                                    acc.push(suffix.render_lark(&mut rules));
                                }

                                format!("({})", acc.join(" "))
                            }
                        };

                        let root = if let Some(Thinking { prefix, suffix }) = thinking {
                            format!(r#"{prefix} "\n" TEXT {suffix} TEXT {tool_call}"#)
                        } else {
                            format!("TEXT {tool_call}")
                        };

                        rules.insert(format!("start: {root}"));

                        Some(rules.into_iter().collect::<Vec<_>>().join("\n"))
                    }
                };

                let prompt = chat_template.render(messages.into(), &validated_tools)?;

                let parser = self.parser();

                Ok(RenderResult {
                    prompt,
                    grammar,
                    parser,
                })
            }
            Config::Harmony => Ok(RenderResult {
                prompt: String::new(),
                grammar: None,
                parser: None,
            }),
        }
    }
}

impl OrderedLiterals {
    pub fn render_lark(&self, rules: &mut HashSet<String>) -> String {
        let OrderedLiterals(literals) = self;

        literals
            .iter()
            .map(|literal| match literal {
                LiteralOrWild::Literal(literal) => lark_string_literal(literal),
                LiteralOrWild::Wild { wild, bounded } => {
                    let lexeme = match wild {
                        WildType::Numeric => {
                            rules.insert(lark::TEXT.to_string());
                            "TEXT".to_string()
                        }
                        WildType::Any => {
                            rules.insert(lark::NUMBER.to_string());
                            "NUMBER".to_string()
                        }
                    };

                    if let Some(bounded) = bounded {
                        format!("{lexeme}{{0,{bounded}}}")
                    } else {
                        format!("{lexeme}*")
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl TemplateTool {
    pub fn naive_json_schema(&self, name_key: &str, argument_key: &str) -> serde_json::Value {
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

impl ToolCall {
    fn render_lark(&self, tools: &[TemplateTool], rules: &mut HashSet<String>) -> String {
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

                lark_json_schema(&object_schema)
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

                lark_json_schema(&array_schema)
            }
            ToolCall::NamedParameters {
                prefix,
                delimiter,
                arguments,
                suffix,
            } => tools
                .iter()
                .map(|tool| {
                    let mut acc = Vec::new();

                    if let Some(prefix) = prefix {
                        acc.push(prefix.render_lark(rules));
                    }

                    acc.push(lark_string_literal(&tool.name));

                    if let Some(delimiter) = delimiter {
                        acc.push(delimiter.render_lark(rules));
                    }

                    match arguments {
                        Arguments::JsonObject => {
                            acc.push(lark_json_schema(&tool.parameters));
                        }
                    }

                    if let Some(suffix) = suffix {
                        acc.push(suffix.render_lark(rules));
                    }

                    acc.join(" ")
                })
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("json schema for tool {0} is invalid: {1}")]
    JsonSchema(String, String),

    #[error("regex for tool {0} is invalid: {1}")]
    Regex(String, String),

    #[error("lark grammar for tool {0} is invalid: {1}")]
    Lark(String, String),

    #[error("chat template render error: {0}")]
    Template(#[from] minijinja::Error),

    #[error("tool choice not found in provided tools")]
    ChatToolChoice,
}
