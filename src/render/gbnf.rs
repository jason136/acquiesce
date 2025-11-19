use std::sync::OnceLock;

use pyo3::{
    Py, PyAny, PyErr, PyResult, Python,
    types::{PyAnyMethods, PyDict, PyModule},
};
use pyo3_ffi::c_str;

use crate::render::RenderError;

pub static TEXT: &str = r#"/[^{](.|\n)*/"#;
pub static NUMBER: &str = "[0-9]";

pub fn gbnf_string_literal(literal: &str) -> String {
    format!(r#""{literal}""#)
}

pub fn gbnf_regex(regex: &str) -> String {
    format!("/{regex}/")
}

pub fn gbnf_json_schema(json_schema: &serde_json::Value) -> Result<String, RenderError> {
    let py_src = c_str!(include_str!("python/json_schema_to_gbnf.py"));

    let grammar = Python::attach(|py| -> PyResult<String> {
        static CONVERTER: OnceLock<(Py<PyAny>, Py<PyAny>)> = OnceLock::new();

        let (converter, json_mod) = CONVERTER.get_or_init(|| {
            (|| {
                let module =
                    PyModule::from_code(py, py_src, c_str!("json_schema_to_gbnf.py"), c_str!(""))?;

                let schema_converter_class = module.getattr("SchemaConverter")?;

                let kwargs = PyDict::new(py);
                kwargs.set_item("prop_order", PyDict::new(py))?;
                kwargs.set_item("allow_fetch", false)?;
                kwargs.set_item("dotall", false)?;
                kwargs.set_item("raw_pattern", false)?;

                let converter = schema_converter_class.call((), Some(&kwargs))?;
                let json_mod = PyModule::import(py, "json")?;

                Ok::<_, PyErr>((converter.into(), json_mod.into()))
            })()
            .unwrap()
        });

        let schema_py = json_mod
            .getattr(py, "loads")?
            .call1(py, (json_schema.to_string(),))?;

        converter.call_method1(py, "visit", (schema_py, ""))?;

        let out = converter.call_method0(py, "format_grammar")?;
        out.extract(py)
    })?;

    Ok(grammar)
}
