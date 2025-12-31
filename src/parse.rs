use crate::{Acquiesce, Config, ToolCall, ToolCalls};

pub(crate) mod literal;

pub struct ToolCallDelta {
    pub index: usize,
    pub delta: String,
}

pub(crate) enum ConsumeResult {
    Consumed,
    Omitted,
    Unconsumed(char),
    Rejected(char, &'static str),
}

pub enum ParseResult {
    Content(String),
    ToolCall(ToolCallDelta),
    Rejected(String, &'static str),
    Complete,
}

pub(crate) struct Consumer(pub Box<dyn FnMut(char) -> ConsumeResult>);

pub(crate) trait DynStatefulParser: Send + Sync {
    fn parse(&mut self, token: String) -> Vec<ParseResult>;
    fn box_clone(&self) -> Box<dyn DynStatefulParser>;
}

impl<T> DynStatefulParser for T
where
    T: FnMut(String) -> Vec<ParseResult> + Send + Sync + Clone + 'static,
{
    fn parse(&mut self, token: String) -> Vec<ParseResult> {
        self(token)
    }

    fn box_clone(&self) -> Box<dyn DynStatefulParser> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn DynStatefulParser> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub(crate) type StatefulParser = Box<dyn DynStatefulParser>;

#[derive(Clone)]
pub struct Parser(pub(crate) StatefulParser);

impl Parser {
    pub fn advance(&mut self, token: String) -> impl Iterator<Item = ParseResult> {
        let Parser(parser) = self;
        parser.parse(token).into_iter()
    }

    // pub fn parse_stream(
    //     mut self,
    //     stream: impl Stream<Item = String>,
    // ) -> impl Stream<Item = Result<String, ParseError>> {
    //     stream.map(move |token| self.consume_char(token))
    // }

    pub fn parse_iter(
        self,
        iter: impl Iterator<Item = String>,
    ) -> impl Iterator<Item = ParseResult> {
        let Parser(mut parser) = self;
        iter.flat_map(move |token| parser.parse(token))
    }
}

impl Acquiesce {
    pub fn parser(&self) -> Option<Parser> {
        match self {
            Config::Components { tool_calls, .. } => match tool_calls.as_ref()? {
                ToolCalls::ToolCall { tool_call } => Some(Parser(tool_call.parser())),
                ToolCalls::ToolCallsSection {
                    prefix,
                    tool_call,
                    suffix,
                } => Some(Parser(tool_call.parser())),
            },
            Config::Harmony => None,
        }
    }
}

impl ToolCall {
    fn parser(&self) -> StatefulParser {
        match self {
            ToolCall::JsonObject {
                name_key,
                argument_key,
            } => todo!(),
            ToolCall::JsonArray {
                name_key,
                argument_key,
            } => todo!(),
            ToolCall::NamedParameters {
                prefix,
                delimiter,
                arguments,
                suffix,
            } => {
                enum NamedParametersState {
                    Prefix(String),
                    Name(String),
                    Delimiter(String),
                    Arguments(StatefulParser),
                    Suffix(String),
                }

                todo!()

                // let arguments_consumer = || match arguments {
                //     Arguments::JsonObject => partial_json_consumer(),
                // };

                // let mut state = NamedParametersState::Prefix(String::new());

                // Parser(Box::new(move |c| match state {
                //     NamedParametersState::Prefix(prefix) => match prefix.consume_char(c) {
                //         ConsumeResult::Captured(c) => {
                //             state = NamedParametersState::Name(c.to_string());
                //         }
                //         ConsumeResult::Unconsumed(c) => {
                //             state = NamedParametersState::Prefix(c.to_string());
                //         }
                //         ConsumeResult::Omitted => {
                //             state = NamedParametersState::Prefix(c.to_string());
                //         }
                //     },
                //     NamedParametersState::Name(name) => match name.consume_char(c) {
                //         ConsumeResult::Captured(c) => {
                //             state = NamedParametersState::Delimiter(c.to_string());
                //         }
                //     },
                //     NamedParametersState::Delimiter(delimiter) => match delimiter.consume_char(c) {
                //         ConsumeResult::Captured(c) => {
                //             state = NamedParametersState::Arguments(arguments_parser());
                //         }
                //     },
                //     NamedParametersState::Arguments(arguments) => {
                //         match arguments.consume_char(c) {}
                //     }
                //     NamedParametersState::Suffix(suffix) => match suffix.consume_char(c) {
                //         ConsumeResult::Captured(c) => {
                //             state = NamedParametersState::Suffix(c.to_string());
                //         }
                //     },
                // }))
            }
        }
    }
}
