//! GBNF-specific formatting utilities

pub static TEXT: &str = r#"/[^{](.|\n)*/"#;

pub fn gbnf_string_literal(literal: &str) -> String {
    let escaped = literal
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!(r#""{escaped}""#)
}

pub fn gbnf_regex(regex: &str) -> String {
    format!("/{regex}/")
}
