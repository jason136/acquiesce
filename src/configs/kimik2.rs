use crate::{AcquiesceRepr, Arguments, Config, Lexeme, Thinking, ToolCall, ToolCalls};

pub fn kimi_k2() -> AcquiesceRepr {
    Config::Components {
        chat_template: (),
        thinking: Some(Thinking {
            prefix: Lexeme::Token("<thinking>".to_string()).into(),
            suffix: Lexeme::Token("</thinking>".to_string()).into(),
        }),
        tool_calls: Some(ToolCalls::ToolCallsSection {
            prefix: Lexeme::Token("<|tool_calls_section_begin|>".to_string()).into(),
            tool_call: ToolCall::NamedParameters {
                prefix: Some(Lexeme::Token("<|tool_call_begin|>functions.".to_string()).into()),
                delimiter: Some(
                    [
                        Lexeme::Text(":".to_string()),
                        Lexeme::Regex {
                            pattern: "[0-9]+".to_string(),
                        },
                        Lexeme::Token("<|tool_call_argument_begin|>".to_string()),
                    ]
                    .as_slice()
                    .into(),
                ),
                arguments: Arguments::JsonObject,
                suffix: Some(Lexeme::Token("<|tool_call_end|>".to_string()).into()),
            },
            suffix: Some(Lexeme::Token("<|tool_calls_section_end|>".to_string()).into()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kimi_k2_config() {
        println!("{}", kimi_k2());
    }
}
