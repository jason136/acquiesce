use std::borrow::Cow;

use chrono::Utc;
use hf_hub::CacheRepo;
use itertools::Itertools;
use minijinja::{Environment, Error, ErrorKind, Template, value::Kwargs};
use minijinja_contrib::pycompat;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;

use crate::{
    InitError,
    json::JsonFormatter,
    render::{
        RenderError,
        schema::{
            ChatAssistantChunk, ChatImageUrl, ChatMessageContent, ChatMessageVariant, ChatMessages,
            ChatTool, ChatToolCall, ChatUserChunk, CustomTool, CustomToolFormat, CustomToolGrammar,
            CustomToolSyntax, FunctionTool,
        },
    },
};

static CHAT_TEMPLATE: &str = "chat_template.jinja";
static TOKENIZER_CONFIG: &str = "tokenizer_config.json";
static MODEL_CONFIG: &str = "config.json";

pub struct ChatTemplate {
    template: Template<'static, 'static>,
    bos_token: Option<String>,
    eos_token: Option<String>,
    multimodal: bool,
    add_generation_prompt: bool,
}

#[derive(Serialize)]
pub struct ChatTemplateInputs<'a> {
    messages: &'a [TemplateChatMessage],
    tools: &'a [TemplateTool],
    bos_token: Option<&'a str>,
    eos_token: Option<&'a str>,
    add_generation_prompt: bool,
}

impl ChatTemplate {
    pub fn from_repo(repo: &CacheRepo) -> Result<Self, InitError> {
        let template_filename = repo.get(CHAT_TEMPLATE);

        let tokenizer_config_string = std::fs::read_to_string(
            repo.get(TOKENIZER_CONFIG)
                .ok_or(InitError::ConfigNotFound(TOKENIZER_CONFIG))?,
        )?;
        let tokenizer_config = serde_json::from_str::<TokenizerConfig>(&tokenizer_config_string)?;

        let model_config_string = std::fs::read_to_string(
            repo.get(MODEL_CONFIG)
                .ok_or(InitError::ConfigNotFound(MODEL_CONFIG))?,
        )?;
        let model_config = serde_json::from_str::<ModelConfig>(&model_config_string)?;

        let multimodal = model_config.image_token_id.is_some();

        let template_string = if let Some(file) = template_filename {
            std::fs::read_to_string(file)?
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

        Self::from_options(
            template_string,
            tokenizer_config.bos_token,
            tokenizer_config.eos_token,
            multimodal,
            true,
        )
    }

    pub fn from_options(
        chat_template: String,
        bos_token: Option<String>,
        eos_token: Option<String>,
        multimodal: bool,
        add_generation_prompt: bool,
    ) -> Result<Self, InitError> {
        let mut environment = Environment::new();
        environment.set_unknown_method_callback(pycompat::unknown_method_callback);

        fn tojson(value: minijinja::Value, kwargs: Kwargs) -> Result<String, Error> {
            let indent: Option<u32> = kwargs.get("indent")?;
            let sort_keys: Option<bool> = kwargs.get("sort_keys")?;
            let ensure_ascii: Option<bool> = kwargs.get("ensure_ascii")?;
            let separators: Option<minijinja::Value> = kwargs.get("separators")?;

            kwargs.assert_all_used()?;

            let (item_separator, key_separator) = if let Some(value) = separators {
                value
                    .try_iter()
                    .map_err(|e| Error::new(ErrorKind::InvalidOperation, e.to_string()))?
                    .map(|v| Cow::Owned(v.to_string()))
                    .collect_tuple()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidOperation,
                            "separators must be a tuple of two strings",
                        )
                    })?
            } else {
                (
                    Cow::Borrowed(if indent.is_some() { "," } else { ", " }),
                    Cow::Borrowed(": "),
                )
            };

            let formatter = JsonFormatter {
                indent_width: indent.map(|n| n as usize),
                item_separator: &item_separator,
                key_separator: &key_separator,
                sort_keys: sort_keys.unwrap_or(false),
                ensure_ascii: ensure_ascii.unwrap_or(true),
                escape_solidus: false,
            };

            formatter
                .serialize(&value)
                .map_err(|e| Error::new(ErrorKind::InvalidOperation, e.to_string()))
        }

        fn raise_exception(err_text: String) -> minijinja::Error {
            minijinja::Error::new(ErrorKind::SyntaxError, err_text)
        }

        fn strftime_now(format_str: &str) -> String {
            Utc::now().format(format_str).to_string()
        }

        environment.add_filter("tojson", tojson);
        environment.add_function("raise_exception", raise_exception);
        environment.add_function("strftime_now", strftime_now);

        let template = Box::leak(Box::new(environment))
            .template_from_str(Box::leak(chat_template.into_boxed_str()))?;

        // let variables = template.undeclared_variables(true);
        // let use_default_tool_template = !variables.contains("tools");

        Ok(Self {
            template,
            bos_token,
            eos_token,
            multimodal,
            add_generation_prompt,
        })
    }

    pub fn render(
        &self,
        mut messages: Vec<TemplateChatMessage>,
        tools: &[TemplateTool],
    ) -> Result<String, RenderError> {
        for message in messages.iter_mut() {
            if self.multimodal {
                if let ChatTemplateContent::Collapsed(text) = &mut message.content {
                    message.content =
                        ChatTemplateContent::Chunks(vec![std::mem::take(text).into()]);
                }
            } else if let ChatTemplateContent::Chunks(chunks) = &mut message.content {
                message.content = ChatTemplateContent::Collapsed(chunks.iter().fold(
                    String::new(),
                    |mut acc, chunk| {
                        if let ChatTemplateChunk::Text { text } = chunk {
                            acc += text;
                        }

                        acc
                    },
                ));
            }
        }

        // let final_message = messages.last().and_then(|msg| {
        //     msg.content.last().and_then(|chunk| {
        //         if let ChatTemplateChunk::Text { text } = chunk {
        //             Some((msg.role.clone(), text.clone()))
        //         } else {
        //             None
        //         }
        //     })
        // });

        let inputs = ChatTemplateInputs {
            messages: &messages,
            tools,
            bos_token: self.bos_token.as_deref(),
            eos_token: self.eos_token.as_deref(),
            add_generation_prompt: true,
        };

        let rendered_template = self.template.render(&inputs)?;

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
    match Option::<TokenizerConfigToken>::deserialize(deserializer)? {
        Some(TokenizerConfigToken::String(s)) => Ok(Some(s)),
        Some(TokenizerConfigToken::Object { content }) => Ok(Some(content)),
        None => Ok(None),
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

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChatTemplateChunk {
    Text { text: String },
    Image { url: String },
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ChatTemplateContent {
    Chunks(Vec<ChatTemplateChunk>),
    Collapsed(String),
}

#[derive(Serialize)]
pub struct TemplateChatMessage {
    pub role: String,
    pub content: ChatTemplateContent,
    pub name: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
    pub tool_call_id: Option<String>,
}

impl From<String> for ChatTemplateChunk {
    fn from(text: String) -> Self {
        ChatTemplateChunk::Text { text }
    }
}

impl From<ChatUserChunk> for ChatTemplateChunk {
    fn from(chunk: ChatUserChunk) -> Self {
        match chunk {
            ChatUserChunk::Text { text } => ChatTemplateChunk::Text { text },
            ChatUserChunk::ImageUrl {
                image_url: ChatImageUrl { url },
            } => ChatTemplateChunk::Image { url },
        }
    }
}

impl From<ChatAssistantChunk> for ChatTemplateChunk {
    fn from(chunk: ChatAssistantChunk) -> Self {
        match chunk {
            ChatAssistantChunk::Text { text } => ChatTemplateChunk::Text { text },
            ChatAssistantChunk::Refusal { refusal } => ChatTemplateChunk::Text { text: refusal },
        }
    }
}

impl<T: Into<ChatTemplateChunk>> From<ChatMessageContent<T>> for Vec<ChatTemplateChunk> {
    fn from(content: ChatMessageContent<T>) -> Self {
        match content {
            ChatMessageContent::SingleText(text) => vec![text.into()],
            ChatMessageContent::ManyChunks(chunks) => chunks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ChatMessages> for Vec<TemplateChatMessage> {
    fn from(messages: ChatMessages) -> Self {
        match messages {
            ChatMessages::Content(s) => {
                vec![TemplateChatMessage {
                    content: ChatTemplateContent::Chunks(
                        ChatMessageContent::<String>::SingleText(s).into(),
                    ),
                    role: "user".to_string(),
                    name: None,
                    refusal: None,
                    tool_calls: None,
                    tool_call_id: None,
                }]
            }
            ChatMessages::Conversation(messages) => messages
                .into_iter()
                .map(|m| match m {
                    ChatMessageVariant::Developer(msg) => TemplateChatMessage {
                        content: ChatTemplateContent::Chunks(msg.content.into()),
                        role: "developer".to_string(),
                        name: msg.name,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    ChatMessageVariant::System(msg) => TemplateChatMessage {
                        content: ChatTemplateContent::Chunks(msg.content.into()),
                        role: "system".to_string(),
                        name: msg.name,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    ChatMessageVariant::User(msg) => TemplateChatMessage {
                        content: ChatTemplateContent::Chunks(msg.content.into()),
                        role: "user".to_string(),
                        name: msg.name,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    ChatMessageVariant::Assistant(msg) => TemplateChatMessage {
                        content: ChatTemplateContent::Chunks(msg.content.into()),
                        role: "assistant".to_string(),
                        name: msg.name,
                        refusal: msg.refusal,
                        tool_calls: msg.tool_calls,
                        tool_call_id: None,
                    },
                    ChatMessageVariant::Tool(msg) => TemplateChatMessage {
                        content: ChatTemplateContent::Chunks(msg.content.into()),
                        role: "tool".to_string(),
                        name: None,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: Some(msg.tool_call_id),
                    },
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub struct TemplateTool {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

impl From<ChatTool> for TemplateTool {
    fn from(value: ChatTool) -> Self {
        match value {
            ChatTool::Function {
                function:
                    FunctionTool {
                        name,
                        description,
                        parameters,
                    },
            } => TemplateTool {
                name,
                description,
                parameters,
            },
            ChatTool::Custom {
                custom:
                    CustomTool {
                        name,
                        description,
                        format,
                    },
            } => TemplateTool {
                name,
                description,
                parameters: match format {
                    CustomToolFormat::Text => json!({ "type": "string" }),
                    CustomToolFormat::Grammar {
                        grammar: CustomToolGrammar { definition, syntax },
                    } => match syntax {
                        CustomToolSyntax::Lark => {
                            json!({ "type": "string", "description": format!("a string that conforms to the following Lark grammar: {}", definition) })
                        }
                        CustomToolSyntax::Regex => {
                            json!({ "type": "string", "pattern": definition })
                        }
                    },
                },
            },
        }
    }
}
