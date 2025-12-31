pub static TEXT: &str = r#"/[^{](.|\n)*/"#;
pub static NUMBER: &str = "/[0-9]/";

pub fn lark_string_literal(literal: &str) -> String {
    format!(r#""{literal}""#)
}

pub fn lark_token_literal(token: &str) -> String {
    token.to_string()
}

pub fn lark_regex(regex: &str) -> String {
    format!("/{regex}/")
}

pub fn lark_json_schema(json_schema: &serde_json::Value) -> String {
    format!("%json {json_schema}")
}
