use std::{
    fmt::{self, Display},
    mem::take,
};

use crate::parse::{ConsumeResult, Consumer};

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
                            *state = ObjectState::Colon(take(&mut key.buffer));
                        }
                        consume_result => return consume_result,
                    },
                    ObjectState::Value(key, value) => match value.consume_char(c) {
                        ConsumeResult::Unconsumed(_) => {
                            entries.push((take(key), take(value)));
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
                            *state = ObjectState::Value(take(key), Box::default());
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
                            elements.push(take(element));
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
                    return ConsumeResult::Rejected(c, literal);
                } else {
                    ConsumeResult::Consumed
                }
            }
        }
    }
}

impl Display for PartialJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartialJson::Start => {}
            PartialJson::Object { entries, state } => {
                write!(f, "{{")?;

                let mut entries_iter = entries.iter().peekable();
                while let Some((key, value)) = entries_iter.next() {
                    write!(f, "\"{key}\":{value}")?;
                    if entries_iter.peek().is_some() {
                        write!(f, ",")?;
                    }
                }

                let comma_prefix = if entries.is_empty() { "" } else { "," };
                match state {
                    ObjectState::Opened => {}
                    ObjectState::Key(key) => write!(f, "{comma_prefix}{key}")?,
                    ObjectState::Colon(key) => write!(f, "{comma_prefix}\"{key}\"")?,
                    ObjectState::Value(key, value) => write!(f, "{comma_prefix}\"{key}\":{value}")?,
                    ObjectState::Comma => {}
                    ObjectState::Closed => write!(f, "}}")?,
                };
            }
            PartialJson::Array { elements, state } => {
                write!(f, "[")?;

                let mut elements_iter = elements.iter().peekable();
                while let Some(element) = elements_iter.next() {
                    write!(f, "{element}")?;
                    if elements_iter.peek().is_some() {
                        write!(f, ",")?;
                    }
                }

                match state {
                    ArrayState::Opened => {}
                    ArrayState::Element(element) => {
                        if !elements.is_empty() {
                            write!(f, ",")?;
                        }
                        write!(f, "{element}")?;
                    }
                    ArrayState::Comma => {}
                    ArrayState::Closed => write!(f, "]")?,
                };
            }
            PartialJson::String(json_string) => write!(f, "{json_string}")?,
            PartialJson::Number { buffer, .. } => write!(f, "{buffer}")?,
            PartialJson::Literal { buffer, .. } => write!(f, "{buffer}")?,
        };

        Ok(())
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

impl Display for JsonString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !matches!(self.state, StringState::Start) {
            write!(f, "\"")?;
        }

        for c in self.buffer.chars() {
            match c {
                c if c.is_control() => write!(f, "\\u{:04x}", c as u32)?,
                '"' => write!(f, "\\\"")?,
                '\\' => write!(f, "\\\\")?,
                '/' => write!(f, "\\/")?,
                '\u{0008}' => write!(f, "\\b")?,
                '\u{000C}' => write!(f, "\\f")?,
                '\n' => write!(f, "\\n")?,
                '\r' => write!(f, "\\r")?,
                '\t' => write!(f, "\\t")?,
                _ => write!(f, "{c}")?,
            }
        }

        if let StringState::Closed = self.state {
            write!(f, "\"")?;
        }

        Ok(())
    }
}
