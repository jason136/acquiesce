use crate::{Acquiesce, Error, ToolCalls};

pub mod schema;

pub enum GrammarLanguage {
    Lark,
}

pub struct RenderResult {
    pub prompt: String,
    pub grammar: Option<String>,
}

impl Acquiesce {
    pub fn render(&self, language: GrammarLanguage) -> Result<RenderResult, RenderError> {
        match self {
            Acquiesce::Components { tool_calls, .. } => {
                let prompt = String::new();

                // let grammar = tool_calls.map(|tool_calls| match tool_calls {
                //     ToolCalls::ToolCall { tool_call } => tool_call.parser(),
                //     ToolCalls::ToolCallsSection {
                //         prefix,
                //         tool_call,
                //         suffix,
                //     } => tool_call.parser(),
                // });

                // Ok(RenderResult { prompt, grammar })

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
pub enum RenderError {}
