use std::sync::{Arc, Mutex};

use acquiesce::{
    parse::{ParseResult, Parser},
    render::{GrammarType, RenderResult},
    Acquiesce, AcquiesceRepr,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct AcquiesceHandle(Acquiesce);

#[napi]
impl AcquiesceHandle {
    #[napi(constructor)]
    pub fn new(
        source: String,
        chat_template: String,
        bos_token: Option<String>,
        eos_token: Option<String>,
    ) -> Result<Self> {
        let repr = serde_json::from_str::<AcquiesceRepr>(&source)
            .or(AcquiesceRepr::infer_default(source.as_str()))
            .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;

        Ok(Self(
            repr.resolve_from_options(chat_template, bos_token, eos_token)
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?,
        ))
    }

    #[napi(ts_return_type = "Promise<RenderTaskResult>")]
    pub fn render<'a>(
        &'a self,
        messages_json: String,
        tools_json: String,
        tool_choice_json: String,
        parallel_tool_calls: bool,
    ) -> AsyncTask<RenderTask<'a>> {
        let AcquiesceHandle(inner) = self;
        AsyncTask::new(RenderTask {
            inner,
            messages_json,
            tools_json,
            tool_choice_json,
            parallel_tool_calls,
            grammar_type: GrammarType::Lark,
        })
    }

    #[napi(ts_return_type = "Promise<ParseTaskResult>")]
    pub fn parse(&self, parser: ExternalRef<Arc<Mutex<Parser>>>) -> AsyncTask<ParseTask> {
        AsyncTask::new(ParseTask {
            parser: parser.clone(),
        })
    }
}

pub struct RenderTask<'a> {
    inner: &'a Acquiesce,
    messages_json: String,
    tools_json: String,
    tool_choice_json: String,
    parallel_tool_calls: bool,
    grammar_type: GrammarType,
}

#[napi(object)]
pub struct RenderTaskResult {
    pub prompt: String,
    pub grammar: Option<String>,
    pub parser: Option<ExternalRef<Arc<Mutex<Parser>>>>,
}

#[napi]
impl<'a> Task for RenderTask<'a> {
    type Output = RenderResult;
    type JsValue = RenderTaskResult;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(RenderResult {
            prompt: self.messages_json.clone(),
            grammar: None,
            parser: None,
        })
    }

    fn resolve(
        &mut self,
        env: Env,
        RenderResult {
            prompt,
            grammar,
            parser,
        }: Self::Output,
    ) -> Result<Self::JsValue> {
        Ok(RenderTaskResult {
            prompt,
            grammar,
            parser: parser
                .map(|p| ExternalRef::new(&env, Arc::new(Mutex::new(p))))
                .transpose()?,
        })
    }
}

pub struct ParseTask {
    parser: Arc<Mutex<Parser>>,
}

#[napi(object)]
pub struct ParseTaskResult {}

#[napi]
impl Task for ParseTask {
    type Output = Vec<ParseResult>;
    type JsValue = ParseTaskResult;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(Vec::new())
    }

    fn resolve(&mut self, _env: Env, _results: Self::Output) -> Result<Self::JsValue> {
        Ok(ParseTaskResult {})
    }
}
