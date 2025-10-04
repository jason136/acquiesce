use crate::{
    Acquiesce, AcquiesceInit, Error,
    render::schema::{ChatMessage, Tool, ToolChoice},
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
        messages: Vec<ChatMessage>,
        tools: Vec<Tool>,
        tool_choice: ToolChoice,
        grammar_type: GrammarType,
    ) -> Result<RenderResult, RenderError> {
        match self {
            Acquiesce::Components {
                chat_template,
                tool_calls,
            } => {
                let prompt = chat_template.render(messages, tools)?;

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
    #[error("chat template render error: {0}")]
    Template(#[from] minijinja::Error),
}
