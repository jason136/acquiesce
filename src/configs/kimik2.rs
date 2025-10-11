use crate::{
    AcquiesceRepr, Arguments, Config, LiteralOrWild, Thinking, ToolCall, ToolCalls, WildType,
};

pub fn kimi_k2() -> AcquiesceRepr {
    Config::Components {
        chat_template: (),
        thinking: Some(Thinking {
            prefix: "<thinking>".into(),
            suffix: "</thinking>".into(),
        }),
        tool_calls: Some(ToolCalls::ToolCallsSection {
            prefix: "<|tool_calls_section_begin|>".into(),
            tool_call: ToolCall::NamedParameters {
                prefix: Some("<|tool_call_begin|>functions.".into()),
                delimiter: Some(
                    [
                        LiteralOrWild::Literal(":".to_string()),
                        LiteralOrWild::Wild {
                            wild: WildType::Numeric,
                            bounded: None,
                        },
                        LiteralOrWild::Literal("<|tool_call_argument_begin|>".to_string()),
                    ]
                    .as_slice()
                    .into(),
                ),
                arguments: Arguments::JsonObject,
                suffix: Some("<|tool_call_end|>".into()),
            },
            suffix: Some("<|tool_calls_section_end|>".into()),
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
