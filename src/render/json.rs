use core::fmt;
use std::fmt::Display;

use serde::Serialize;
use serde_json::Value;

pub struct JsonFormat<'a> {
    pub indent: Option<usize>,
    pub key_separator: &'a str,
    pub item_separator: &'a str,
    pub sort_keys: bool,
    pub ensure_ascii: bool,
}

impl<'a> Default for JsonFormat<'a> {
    fn default() -> Self {
        Self {
            indent: None,
            key_separator: ":",
            item_separator: ",",
            sort_keys: false,
            ensure_ascii: false,
        }
    }
}

impl<'a> JsonFormat<'a> {
    pub fn pretty(indent: usize) -> Self {
        Self {
            indent: Some(indent),
            ..Default::default()
        }
    }

    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<String, serde_json::Error> {
        let json_value = serde_json::to_value(value)?;

        Ok(JsonFormatState {
            value: &json_value,
            format: self,
            depth: 0,
        }
        .to_string())
    }
}

struct JsonFormatState<'a> {
    value: &'a Value,
    format: &'a JsonFormat<'a>,
    depth: usize,
}

impl<'a> Display for JsonFormatState<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => Value::String(s.to_string()).fmt(f),
            Value::Array(arr) => {
                out.push('[');
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push_str(item_sep);
                    }
                    newline_indent(out, depth + 1, indent);
                    write_value(v, out, depth + 1, indent, item_sep, key_sep);
                }
                if !arr.is_empty() {
                    newline_indent(out, depth, indent);
                }
                out.push(']');
            }
            Value::Object(obj) => {
                out.push('{');
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 {
                        out.push_str(item_sep);
                    }
                    newline_indent(out, depth + 1, indent);
                    write_string(k, out);
                    out.push_str(key_sep);
                    write_value(v, out, depth + 1, indent, item_sep, key_sep);
                }
                if !obj.is_empty() {
                    newline_indent(out, depth, indent);
                }
                out.push('}');
            }
        }

        Ok(())
    }
}

fn escape_non_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            for unit in c.encode_utf16(&mut [0; 2]) {
                out.push_str(&format!("\\u{:04x}", unit));
            }
        }
    }

    out
}

struct AsciiString(String);

impl Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c.is_ascii() {
                write!(f, "{c}")?;
            } else {
                write!(f, "\\u{:04x}", c as u32)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default() {
        assert_eq!(
            dumps(&json!({"a": 1, "b": 2}), false, None, None, false).unwrap(),
            r#"{"a": 1, "b": 2}"#
        );
    }

    #[test]
    fn test_compact_separators() {
        assert_eq!(
            dumps(&json!({"a": 1}), false, None, Some((",", ":")), false).unwrap(),
            r#"{"a":1}"#
        );
    }

    #[test]
    fn test_indent() {
        assert_eq!(
            dumps(&json!({"a": 1}), false, Some(2), None, false).unwrap(),
            "{\n  \"a\": 1\n}"
        );
    }

    #[test]
    fn test_sort_keys() {
        assert_eq!(
            dumps(
                &json!({"b": 2, "a": 1}),
                false,
                None,
                Some((",", ":")),
                true
            )
            .unwrap(),
            r#"{"a":1,"b":2}"#
        );
    }

    #[test]
    fn test_ensure_ascii() {
        assert_eq!(
            dumps(&json!("café"), true, None, None, false).unwrap(),
            r#""\u0063\u0061\u0066\u00e9""#
        );
    }

    #[test]
    fn test_nested_sort() {
        assert_eq!(
            dumps(
                &json!({"z": {"b": 1, "a": 2}}),
                false,
                None,
                Some((",", ":")),
                true
            )
            .unwrap(),
            r#"{"z":{"a":2,"b":1}}"#
        );
    }
}
