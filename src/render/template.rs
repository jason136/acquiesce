use std::path::Path;

use chrono::Utc;
use minijinja::{Environment, ErrorKind, Template};
use minijinja_contrib::pycompat;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    InitError,
    render::{
        RenderError,
        schema::{TemplateChatMessage, TemplateTool},
    },
};

static CHAT_TEMPLATE: &str = "chat_template.jinja";
static TOKENIZER_CONFIG: &str = "tokenizer_config.json";
static MODEL_CONFIG: &str = "config.json";

pub struct ChatTemplate {
    template: Template<'static, 'static>,
    bos_token: Option<String>,
    eos_token: Option<String>,
    use_default_tool_template: bool,
    multimodal: bool,
}

#[derive(Serialize)]
pub struct ChatTemplateInputs<'a> {
    messages: Vec<TemplateChatMessage>,
    tools: Vec<TemplateTool>,
    bos_token: Option<&'a str>,
    eos_token: Option<&'a str>,
    add_generation_prompt: bool,
}

impl ChatTemplate {
    pub fn from_repo(dir: &Path) -> Result<Self, InitError> {
        let template_filename = dir.join(CHAT_TEMPLATE);

        let tokenizer_config_string = std::fs::read_to_string(dir.join(TOKENIZER_CONFIG))?;
        let tokenizer_config = serde_json::from_str::<TokenizerConfig>(&tokenizer_config_string)?;

        let model_config_string = std::fs::read_to_string(dir.join(MODEL_CONFIG))?;
        let model_config = serde_json::from_str::<ModelConfig>(&model_config_string)?;

        let multimodal = model_config.image_token_id.is_some();

        let template_string = if template_filename.is_file() {
            std::fs::read_to_string(template_filename)?
        } else if let Some(template_string) = tokenizer_config.chat_template.and_then(|c| match c {
            ChatTemplaces::Single(template) => Some(template),
            ChatTemplaces::Named(templates) => templates
                .iter()
                .find(|t| t.name == "default")
                .or_else(|| templates.first())
                .map(|t| t.template.clone()),
        }) {
            template_string
        } else {
            return Err(InitError::MissingTemplate);
        };

        let mut environment = Environment::new();
        environment.set_unknown_method_callback(pycompat::unknown_method_callback);

        fn raise_exception(err_text: String) -> minijinja::Error {
            minijinja::Error::new(ErrorKind::SyntaxError, err_text)
        }

        environment.add_function("raise_exception", raise_exception);

        fn strftime_now(format_str: &str) -> String {
            Utc::now().format(format_str).to_string()
        }

        environment.add_function("strftime_now", strftime_now);

        let template = Box::leak(Box::new(environment))
            .template_from_str(Box::leak(template_string.into_boxed_str()))?;

        let variables = template.undeclared_variables(true);
        let use_default_tool_template = !variables.contains("tools");

        Ok(Self {
            template,
            bos_token: tokenizer_config.bos_token,
            eos_token: tokenizer_config.eos_token,
            use_default_tool_template,
            multimodal,
        })
    }

    pub fn render(
        &self,
        messages: Vec<TemplateChatMessage>,
        tools: Vec<TemplateTool>,
    ) -> Result<String, RenderError> {
        // let final_message = messages.last().and_then(|msg| {
        //     msg.content.last().and_then(|chunk| {
        //         if let ChatMessageChunk::Text { text } = chunk {
        //             Some((msg.role.clone(), text.clone()))
        //         } else {
        //             None
        //         }
        //     })
        // });

        let inputs = ChatTemplateInputs {
            messages,
            tools,
            bos_token: self.bos_token.as_deref(),
            eos_token: self.eos_token.as_deref(),
            add_generation_prompt: true,
        };

        let rendered_template = self.template.render(inputs)?;

        // match final_message {
        //     Some((role, text)) if role == "assistant" => {
        //         if let Some(index) = rendered_template.rfind(&text) {
        //             return Ok(rendered_template[..index + text.len()]
        //                 .trim_end()
        //                 .to_string());
        //         }
        //     }
        //     _ => {}
        // }

        Ok(rendered_template)
    }
}

#[derive(Deserialize)]
pub struct NamedChatTemplate {
    name: String,
    template: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ChatTemplaces {
    Single(String),
    Named(Vec<NamedChatTemplate>),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum TokenizerConfigToken {
    String(String),
    Object { content: String },
}

fn deserialize_config_token<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match TokenizerConfigToken::deserialize(deserializer)? {
        TokenizerConfigToken::String(s) => Ok(Some(s)),
        TokenizerConfigToken::Object { content } => Ok(Some(content)),
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct TokenizerConfig {
    pub chat_template: Option<ChatTemplaces>,
    pub completion_template: Option<String>,
    #[serde(deserialize_with = "deserialize_config_token")]
    pub bos_token: Option<String>,
    #[serde(deserialize_with = "deserialize_config_token")]
    pub eos_token: Option<String>,
    pub tokenizer_class: Option<String>,
    pub add_bos_token: Option<bool>,
    pub add_eos_token: Option<bool>,
    pub guideline: Option<String>,
}

#[derive(Deserialize)]
pub struct ModelConfig {
    pub image_token_id: Option<u32>,
}
