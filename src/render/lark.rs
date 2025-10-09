pub static TEXT: &str = r#"TEXT: /[^{](.|\n)*/"#;

pub static NUMBER: &str = "NUMBER: /[0-9]+/";

pub static JSON_CHAR: &str =
    r#"JSON_CHAR: /(\\([\"\\\/bfnrt]|u[a-fA-F0-9]{4})|[^\"\\\x00-\x1F\x7F])/"#;

pub static JSON_STRING: &str = r#"JSON_STRING: "\"" JSON_CHAR* "\""#;

pub fn lark_string_literal(literal: &str) -> String {
    format!(r#""{literal}""#)
}

pub fn lark_regex(regex: &str) -> String {
    format!("/{regex}/")
}

pub fn lark_json_schema(json_schema: &serde_json::Value) -> String {
    format!("%json {json_schema}")
}

// pub fn lark_
