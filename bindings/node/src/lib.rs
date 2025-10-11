use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use acquiesce::{
    configs::kimik2::kimi_k2,
    parse::{ParseResult, Parser},
    render::{GrammarType, RenderResult},
    Acquiesce,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct AcquiesceHandle(Acquiesce);

#[napi]
impl AcquiesceHandle {
    #[napi(constructor)]
    pub fn from_repo_with_fallback(path: String, fallback_name: Option<String>) -> Result<Self> {
        let inner = if let Some(fallback_name) = fallback_name {
            let fallback = match fallback_name.as_str() {
                "kimi" => kimi_k2(),
                _ => return Err(Error::new(Status::InvalidArg, "Invalid fallback name")),
            };

            Acquiesce::from_repo_with_fallback(Path::new(&path), fallback)
        } else {
            Acquiesce::from_repo(Path::new(&path))
        }
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(Self(inner))
    }

    #[napi]
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

    #[napi]
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
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(Vec::new())
    }

    fn resolve(&mut self, env: Env, results: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}
