use acquiesce::render::{
    GrammarType,
    schema::{ChatMessages, ChatTool, ChatToolChoice},
};
use hf_hub::{Cache, Repo, RepoType};
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyType;
use pyo3_stub_gen::define_stub_info_gatherer;
use pyo3_stub_gen::derive::*;

pyo3::create_exception!(acquiesce_py, InitError, PyValueError);
pyo3::create_exception!(acquiesce_py, RenderError, PyRuntimeError);
pyo3::create_exception!(acquiesce_py, ParseError, PyIOError);

#[gen_stub_pyclass]
#[pyclass]
pub struct Acquiesce(acquiesce::Acquiesce);

#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Parser(acquiesce::parse::Parser);

#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RenderResult {
    #[pyo3(get)]
    pub prompt: String,
    #[pyo3(get)]
    pub grammar: Option<String>,
    #[pyo3(get)]
    pub parser: Option<Parser>,
}

#[gen_stub_pymethods]
#[pymethods]
impl Acquiesce {
    #[classmethod]
    fn from_repo(
        _cls: &Bound<'_, PyType>,
        hf_cache_path: &str,
        model_id: &str,
        revision: Option<&str>,
    ) -> PyResult<Self> {
        let cache = Cache::new(hf_cache_path.into());

        let repo = if let Some(revision) = revision {
            Repo::with_revision(model_id.into(), RepoType::Model, revision.into())
        } else {
            Repo::model(model_id.into())
        };

        let acquiesce = acquiesce::Acquiesce::from_repo(&cache.repo(repo))
            .map_err(|e| InitError::new_err(e.to_string()))?;

        Ok(Self(acquiesce))
    }

    fn render(
        &self,
        py: Python,
        messages_json: String,
        tools_json: String,
        tool_choice_json: String,
        parallel_tool_calls: bool,
    ) -> PyResult<RenderResult> {
        let Acquiesce(inner) = self;
        py.detach(|| {
            let messages = serde_json::from_str::<ChatMessages>(&messages_json)
                .map_err(|e| PyValueError::new_err(format!("Invalid messages JSON: {}", e)))?;
            let tools = serde_json::from_str::<Vec<ChatTool>>(&tools_json)
                .map_err(|e| PyValueError::new_err(format!("Invalid tools JSON: {}", e)))?;
            let tool_choice = serde_json::from_str::<ChatToolChoice>(&tool_choice_json)
                .map_err(|e| PyValueError::new_err(format!("Invalid tool_choice JSON: {}", e)))?;

            let result = inner
                .render(
                    messages,
                    tools,
                    tool_choice,
                    parallel_tool_calls,
                    GrammarType::Lark,
                )
                .map_err(|e| RenderError::new_err(e.to_string()))?;

            Ok(RenderResult {
                prompt: result.prompt,
                grammar: result.grammar,
                parser: result.parser.map(Parser),
            })
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Parser {
    fn parse(&self, py: Python, _text: String) -> PyResult<Vec<String>> {
        let Parser(inner) = self;

        py.detach(|| {
            // let result = inner.parse(_text).map_err(|e| PyParseError::new_err(e.to_string()))?;

            Ok(vec![])
        })
    }
}

#[pymodule]
fn acquiesce_py(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Acquiesce>()?;
    m.add_class::<Parser>()?;
    m.add_class::<RenderResult>()?;
    m.add("InitError", py.get_type::<InitError>())?;
    m.add("RenderError", py.get_type::<RenderError>())?;
    m.add("ParseError", py.get_type::<ParseError>())?;
    Ok(())
}

define_stub_info_gatherer!(stub_info);
