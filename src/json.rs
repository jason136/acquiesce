use std::fmt::{self, Display};

use itertools::{Either, Itertools};
use serde::Serialize;
use serde_json::Value;

use crate::parse::{ConsumeResult, Consumer};

pub struct JsonFormatter<'a> {
    pub indent_width: Option<usize>,
    pub key_separator: &'a str,
    pub item_separator: &'a str,
    pub sort_keys: bool,
    pub ensure_ascii: bool,
    pub escape_solidus: bool,
}

impl<'a> Default for JsonFormatter<'a> {
    fn default() -> Self {
        Self {
            indent_width: None,
            key_separator: ": ",
            item_separator: ", ",
            sort_keys: false,
            ensure_ascii: false,
            escape_solidus: false,
        }
    }
}

impl<'a> JsonFormatter<'a> {
    pub fn pretty(indent_width: usize) -> Self {
        Self {
            indent_width: Some(indent_width),
            item_separator: ",",
            ..Default::default()
        }
    }

    pub fn compact() -> Self {
        Self {
            key_separator: ":",
            item_separator: ",",
            ..Default::default()
        }
    }

    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<String, serde_json::Error> {
        let json_value = serde_json::to_value(value)?;

        Ok(JsonFormatterState {
            value: &json_value,
            format: self,
            depth: 0,
        }
        .to_string())
    }
}

struct JsonFormatterState<'a> {
    value: &'a Value,
    format: &'a JsonFormatter<'a>,
    depth: usize,
}

struct JsonStringFormatter<'a> {
    value: &'a str,
    format: &'a JsonFormatter<'a>,
}

impl<'a> Display for JsonStringFormatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;

        for c in self.value.chars() {
            match c {
                '"' => write!(f, "\\\"")?,
                '\\' => write!(f, "\\\\")?,
                '/' if self.format.escape_solidus => write!(f, "\\/")?,
                '\u{0008}' => write!(f, "\\b")?,
                '\u{000C}' => write!(f, "\\f")?,
                '\n' => write!(f, "\\n")?,
                '\r' => write!(f, "\\r")?,
                '\t' => write!(f, "\\t")?,
                c if c.is_control() => write!(f, "\\u{:04x}", c as u32)?,
                c if !c.is_ascii() && self.format.ensure_ascii => {
                    let mut buf = [0u16; 2];
                    for codepoint in c.encode_utf16(&mut buf) {
                        write!(f, "\\u{codepoint:04x}")?;
                    }
                }
                _ => write!(f, "{c}")?,
            }
        }

        write!(f, "\"")?;

        Ok(())
    }
}

impl<'a> Display for JsonFormatterState<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Value::String(s) => JsonStringFormatter {
                value: s,
                format: self.format,
            }
            .fmt(f),
            Value::Object(entries) => {
                if entries.is_empty() {
                    write!(f, "{{}}")?;

                    return Ok(());
                }

                let Whitespace {
                    newline,
                    inner,
                    outer,
                } = self.whitespace();

                write!(
                    f,
                    "{{{}{newline}{outer}}}",
                    if self.format.sort_keys {
                        Either::Left(entries.iter().sorted_by_key(|(key, _)| key.as_str()))
                    } else {
                        Either::Right(entries.iter())
                    }
                    .format_with(
                        self.format.item_separator,
                        |(key, value), f| {
                            let key_formatter = JsonStringFormatter {
                                value: key,
                                format: self.format,
                            };
                            let value_formatter = JsonFormatterState {
                                value,
                                format: self.format,
                                depth: self.depth + 1,
                            };

                            f(&format_args!(
                                "{newline}{inner}{key_formatter}{}{value_formatter}",
                                self.format.key_separator
                            ))
                        }
                    )
                )
            }
            Value::Array(elements) => {
                if elements.is_empty() {
                    write!(f, "[]")?;

                    return Ok(());
                }

                let Whitespace {
                    newline,
                    inner,
                    outer,
                } = self.whitespace();

                write!(
                    f,
                    "[{}{newline}{outer}]",
                    elements
                        .iter()
                        .format_with(self.format.item_separator, |value, f| {
                            let element_formatter = JsonFormatterState {
                                value,
                                format: self.format,
                                depth: self.depth + 1,
                            };

                            f(&format_args!("{newline}{inner}{element_formatter}"))
                        }),
                )
            }
            _ => self.value.fmt(f),
        }
    }
}

struct Whitespace<T: Display> {
    newline: &'static str,
    inner: T,
    outer: T,
}

impl<'a> JsonFormatterState<'a> {
    fn whitespace(&self) -> Whitespace<impl Display> {
        struct Padding(usize);

        impl Display for Padding {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:width$}", "", width = self.0)
            }
        }

        if let Some(indent_width) = self.format.indent_width {
            Whitespace {
                newline: "\n",
                inner: Padding(indent_width * (self.depth + 1)),
                outer: Padding(indent_width * self.depth),
            }
        } else {
            Whitespace {
                newline: "",
                inner: Padding(0),
                outer: Padding(0),
            }
        }
    }
}

pub fn partial_json_consumer() -> Consumer {
    let mut state = PartialJson::default();

    Consumer(Box::new(move |c| state.consume_char(c)))
}

#[derive(Default)]
pub enum PartialJson {
    #[default]
    Start,
    Object {
        entries: Vec<(String, PartialJson)>,
        state: ObjectState,
    },
    Array {
        elements: Vec<PartialJson>,
        state: ArrayState,
    },
    String(JsonString),
    Number {
        buffer: String,
        state: NumberState,
    },
    Literal {
        buffer: String,
        literal: &'static str,
    },
}

pub enum ObjectState {
    Opened,
    Key(JsonString),
    Colon(String),
    Value(String, Box<PartialJson>),
    Comma,
    Closed,
}

pub enum ArrayState {
    Opened,
    Element(Box<PartialJson>),
    Comma,
    Closed,
}

pub struct JsonString {
    buffer: String,
    state: StringState,
}

pub enum StringState {
    Start,
    Opened,
    Escaped,
    HexDigits(Vec<char>),
    Closed,
}

pub enum NumberState {
    OpenedPositive,
    OpenedZero,
    OpenedNegative,
    FirstDecimal,
    Decimal,
    ExponentSign,
    FirstExponent,
    Exponent,
}

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

pub struct PartialJsonError {
    pub c: char,
    pub expected: &'static str,
}

impl PartialJson {
    pub fn consume_char(&mut self, c: char) -> ConsumeResult {
        match self {
            PartialJson::Start => {
                match c {
                    c if is_whitespace(c) => return ConsumeResult::Omitted,
                    '{' => {
                        *self = PartialJson::Object {
                            entries: Vec::new(),
                            state: ObjectState::Opened,
                        };
                    }
                    '[' => {
                        *self = PartialJson::Array {
                            elements: Vec::new(),
                            state: ArrayState::Opened,
                        };
                    }
                    '"' => {
                        *self = PartialJson::String(JsonString {
                            buffer: String::new(),
                            state: StringState::Opened,
                        });
                    }
                    '1'..='9' => {
                        *self = PartialJson::Number {
                            buffer: c.to_string(),
                            state: NumberState::OpenedPositive,
                        };
                    }
                    '0' => {
                        *self = PartialJson::Number {
                            buffer: c.to_string(),
                            state: NumberState::OpenedZero,
                        };
                    }
                    '-' => {
                        *self = PartialJson::Number {
                            buffer: c.to_string(),
                            state: NumberState::OpenedNegative,
                        };
                    }
                    't' => {
                        *self = PartialJson::Literal {
                            buffer: c.to_string(),
                            literal: "true",
                        }
                    }
                    'f' => {
                        *self = PartialJson::Literal {
                            buffer: c.to_string(),
                            literal: "false",
                        }
                    }
                    'n' => {
                        *self = PartialJson::Literal {
                            buffer: c.to_string(),
                            literal: "null",
                        }
                    }
                    _ => return ConsumeResult::Rejected(c, "a valid json start character"),
                }

                ConsumeResult::Consumed
            }
            PartialJson::Object { entries, state } => {
                match state {
                    ObjectState::Key(key) => match key.consume_char(c) {
                        ConsumeResult::Unconsumed(_) => {
                            *state = ObjectState::Colon(std::mem::take(&mut key.buffer));
                        }
                        consume_result => return consume_result,
                    },
                    ObjectState::Value(key, value) => match value.consume_char(c) {
                        ConsumeResult::Unconsumed(_) => {
                            entries.push((std::mem::take(key), std::mem::take(value)));
                            *state = ObjectState::Comma;
                        }
                        consume_result => return consume_result,
                    },
                    _ => {}
                }

                match state {
                    ObjectState::Opened => match c {
                        c if is_whitespace(c) => return ConsumeResult::Omitted,
                        '}' => {
                            *state = ObjectState::Closed;
                        }
                        '"' => {
                            let key = JsonString {
                                buffer: String::new(),
                                state: StringState::Opened,
                            };
                            *state = ObjectState::Key(key);
                        }
                        _ => {
                            return ConsumeResult::Rejected(
                                c,
                                "a valid json object start character",
                            );
                        }
                    },
                    ObjectState::Key(..) => { /* handled above */ }
                    ObjectState::Colon(key) => match c {
                        c if is_whitespace(c) => return ConsumeResult::Omitted,
                        ':' => {
                            *state = ObjectState::Value(std::mem::take(key), Box::default());
                        }
                        _ => {
                            return ConsumeResult::Rejected(
                                c,
                                "a valid json object key value separator",
                            );
                        }
                    },
                    ObjectState::Value(..) => { /* handled above */ }
                    ObjectState::Comma => match c {
                        c if is_whitespace(c) => return ConsumeResult::Omitted,
                        ',' => {
                            *state = ObjectState::Key(JsonString {
                                buffer: String::new(),
                                state: StringState::Start,
                            });
                        }
                        '}' => {
                            *state = ObjectState::Closed;
                        }
                        _ => {
                            return ConsumeResult::Rejected(
                                c,
                                "a valid json object entry separator",
                            );
                        }
                    },
                    ObjectState::Closed => return ConsumeResult::Unconsumed(c),
                }

                ConsumeResult::Consumed
            }
            PartialJson::Array { elements, state } => {
                if let ArrayState::Element(element) = state {
                    match element.consume_char(c) {
                        ConsumeResult::Unconsumed(_) => {
                            elements.push(std::mem::take(element));
                            *state = ArrayState::Comma;
                        }
                        consume_result => return consume_result,
                    }
                }

                match state {
                    ArrayState::Opened => match c {
                        c if is_whitespace(c) => return ConsumeResult::Omitted,
                        ']' => {
                            *state = ArrayState::Closed;
                        }
                        _ => {
                            let mut element = PartialJson::default();
                            let consume_result = element.consume_char(c);
                            *state = ArrayState::Element(Box::new(element));
                            return consume_result;
                        }
                    },
                    ArrayState::Element(..) => { /* handled above */ }
                    ArrayState::Comma => match c {
                        c if is_whitespace(c) => return ConsumeResult::Omitted,
                        ',' => {
                            *state = ArrayState::Element(Box::default());
                        }
                        ']' => {
                            *state = ArrayState::Closed;
                        }
                        _ => {
                            return ConsumeResult::Rejected(
                                c,
                                "a valid json array element separator",
                            );
                        }
                    },
                    ArrayState::Closed => return ConsumeResult::Unconsumed(c),
                }

                ConsumeResult::Consumed
            }
            PartialJson::String(json_string) => json_string.consume_char(c),
            PartialJson::Number { buffer, state } => {
                match state {
                    NumberState::OpenedPositive => match c {
                        '0'..='9' => {}
                        '.' => *state = NumberState::FirstDecimal,
                        'e' | 'E' => *state = NumberState::ExponentSign,
                        _ => return ConsumeResult::Unconsumed(c),
                    },
                    NumberState::OpenedZero => match c {
                        '.' => *state = NumberState::FirstDecimal,
                        'e' | 'E' => *state = NumberState::ExponentSign,
                        '0'..='9' => return ConsumeResult::Rejected(c, "a dot or an exponent"),
                        _ => return ConsumeResult::Unconsumed(c),
                    },
                    NumberState::OpenedNegative => match c {
                        '1'..='9' => *state = NumberState::OpenedPositive,
                        '0' => *state = NumberState::OpenedZero,
                        _ => return ConsumeResult::Rejected(c, "a digit"),
                    },
                    NumberState::FirstDecimal => match c {
                        '0'..='9' => *state = NumberState::Decimal,
                        _ => return ConsumeResult::Rejected(c, "a digit"),
                    },
                    NumberState::Decimal => match c {
                        '0'..='9' => {}
                        'e' | 'E' => *state = NumberState::ExponentSign,
                        _ => return ConsumeResult::Unconsumed(c),
                    },
                    NumberState::ExponentSign => match c {
                        '+' | '-' => *state = NumberState::FirstExponent,
                        '0'..='9' => *state = NumberState::Exponent,
                        _ => return ConsumeResult::Rejected(c, "a sign or a digit"),
                    },
                    NumberState::FirstExponent => match c {
                        '0'..='9' => *state = NumberState::Exponent,
                        _ => return ConsumeResult::Rejected(c, "a digit"),
                    },
                    NumberState::Exponent => match c {
                        '0'..='9' => {}
                        _ => return ConsumeResult::Unconsumed(c),
                    },
                }
                buffer.push(c);

                ConsumeResult::Consumed
            }
            PartialJson::Literal {
                buffer, literal, ..
            } => {
                if buffer.len() == literal.len() {
                    return ConsumeResult::Unconsumed(c);
                }

                buffer.push(c);
                if !literal.starts_with(&*buffer) {
                    buffer.pop();
                    ConsumeResult::Rejected(c, literal)
                } else {
                    ConsumeResult::Consumed
                }
            }
        }
    }
}

impl JsonString {
    pub fn consume_char(&mut self, c: char) -> ConsumeResult {
        match &mut self.state {
            StringState::Start => match c {
                c if is_whitespace(c) => return ConsumeResult::Omitted,
                '"' => {
                    self.state = StringState::Opened;
                }
                _ => return ConsumeResult::Rejected(c, "a valid json string start character"),
            },
            StringState::Opened => match c {
                '"' => {
                    self.state = StringState::Closed;
                }
                '\\' => {
                    self.state = StringState::Escaped;
                    return ConsumeResult::Omitted;
                }
                c if c.is_control() => {
                    return ConsumeResult::Rejected(c, "not a raw control character");
                }
                _ => {
                    self.buffer.push(c);
                }
            },
            StringState::Escaped => {
                let escape_char = match c {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'b' => '\u{0008}',
                    'f' => '\u{000C}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'u' => {
                        self.state = StringState::HexDigits(Vec::new());
                        return ConsumeResult::Omitted;
                    }
                    _ => return ConsumeResult::Rejected(c, "a valid json escape character"),
                };
                self.buffer.push(escape_char);
                self.state = StringState::Opened;
                return ConsumeResult::Consumed;
            }
            StringState::HexDigits(hex_digits) => {
                if c.is_ascii_hexdigit() {
                    hex_digits.push(c);
                    if hex_digits.len() == 4 {
                        if let Ok(code_point) =
                            u32::from_str_radix(&hex_digits.iter().collect::<String>(), 16)
                        {
                            if let Some(unicode_char) = char::from_u32(code_point) {
                                self.buffer.push(unicode_char);
                                return ConsumeResult::Consumed;
                            } else {
                                return ConsumeResult::Rejected(c, "a valid unicode code point");
                            }
                        }
                        self.state = StringState::Opened;
                    }

                    return ConsumeResult::Omitted;
                } else {
                    return ConsumeResult::Rejected(c, "valid hex digits for unicode");
                }
            }
            StringState::Closed => return ConsumeResult::Unconsumed(c),
        };

        ConsumeResult::Consumed
    }
}
