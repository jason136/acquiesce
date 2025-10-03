use crate::{Acquiesce, Error};

pub enum Grammar {
    Lark(String),
}

pub struct RenderResult {
    pub prompt: String,
    pub grammar: Option<Grammar>,
}

impl Acquiesce {
    pub fn render(&self) -> Result<RenderResult, RenderError> {
        todo!()
    }
}

#[derive(Debug, Error)]
pub enum RenderError {}
