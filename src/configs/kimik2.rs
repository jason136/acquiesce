use crate::{Acquiesce, Arguments, LiteralOrWild, ToolCall, ToolCalls, WildType, default_roles};

pub fn kimi_k2() -> Acquiesce {
    Acquiesce::Components {
        allowed_roles: default_roles(),
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
    use std::str::FromStr;

    use super::*;

    #[test]
    fn kimi_k2_config() {
        let config = kimi_k2();
        let string_config = config.to_string();
        println!("{string_config}");
        let _ = Acquiesce::from_str(&string_config).unwrap();
    }
}
