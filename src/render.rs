use std::sync::OnceLock;

use llguidance::{ParserFactory, api::TopLevelGrammar, toktrie::ApproximateTokEnv};

use crate::{
    Acquiesce, AcquiesceInit, Error,
    render::schema::{
        ChatMessages, ChatTool, CustomTool, CustomToolFormat, CustomToolGrammar, CustomToolSyntax,
        FunctionTool, ToolChoice,
    },
};

pub(crate) mod template;

pub mod schema;

pub enum GrammarType {
    Lark,
}

pub struct RenderResult {
    pub prompt: String,
    pub grammar: Option<String>,
}

impl AcquiesceInit {
    pub fn render(
        &self,
        messages: ChatMessages,
        tools: Vec<ChatTool>,
        tool_choice: ToolChoice,
        parallel_tool_calls: bool,
        grammar_type: GrammarType,
    ) -> Result<RenderResult, RenderError> {
        match self {
            Acquiesce::Components {
                chat_template,
                tool_calls,
            } => {
                let validated_tools = tools.into_iter().try_fold(Vec::new(), |mut acc, tool| {
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

                                    let grammar = TopLevelGrammar::from_lark(definition.clone());
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

                    acc.push(tool.into());

                    Ok::<_, RenderError>(acc)
                })?;

                let prompt = chat_template.render(messages.into(), validated_tools)?;

                // let grammar = tool_calls.map(|tool_calls| match tool_calls {
                //     ToolCalls::ToolCall { tool_call } => tool_call.parser(),
                //     ToolCalls::ToolCallsSection {
                //         prefix,
                //         tool_call,
                //         suffix,
                //     } => tool_call.parser(),
                // });

                Ok(RenderResult {
                    prompt,
                    grammar: None,
                })
            }
            Acquiesce::Harmony => Ok(RenderResult {
                prompt: String::new(),
                grammar: None,
            }),
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
}
