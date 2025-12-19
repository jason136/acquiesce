use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatImageUrl {
    pub url: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChatUserChunk {
    Text { text: String },
    ImageUrl { image_url: ChatImageUrl },
    // InputAudio { input_audio: }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChatAssistantChunk {
    Text { text: String },
    Refusal { refusal: String },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageContent<T> {
    SingleText(String),
    ManyChunks(Vec<T>),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallType {
    Function,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    pub r#type: Option<ToolCallType>,
    pub function: ChatFunction,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatSystemDeveloperMessage {
    pub content: ChatMessageContent<String>,
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatUserMessage {
    pub content: ChatMessageContent<ChatUserChunk>,
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatAssistantMessage {
    pub content: ChatMessageContent<ChatAssistantChunk>,
    pub refusal: Option<String>,
    pub name: Option<String>,
    // pub audio: Option<Vec<u8>>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatToolMessage {
    pub content: ChatMessageContent<String>,
    pub tool_call_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessageVariant {
    Developer(ChatSystemDeveloperMessage),
    System(ChatSystemDeveloperMessage),
    User(ChatUserMessage),
    Assistant(ChatAssistantMessage),
    Tool(ChatToolMessage),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessages {
    Content(String),
    Conversation(Vec<ChatMessageVariant>),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FunctionTool {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomToolSyntax {
    Lark,
    Regex,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CustomToolGrammar {
    pub definition: String,
    pub syntax: CustomToolSyntax,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum CustomToolFormat {
    Text,
    Grammar { grammar: CustomToolGrammar },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CustomTool {
    pub name: String,
    pub description: Option<String>,
    pub format: CustomToolFormat,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChatTool {
    Function { function: FunctionTool },
    Custom { custom: CustomTool },
}

#[derive(Clone, Deserialize)]
pub struct FunctionName {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
enum TypedChoice {
    Function { function: FunctionName },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ToolChoiceRepr {
    String(String),
    TypedChoice(TypedChoice),
}

impl From<ToolChoiceRepr> for ChatToolChoice {
    fn from(value: ToolChoiceRepr) -> Self {
        match value {
            ToolChoiceRepr::String(s) => match s.as_str() {
                "none" => ChatToolChoice::None,
                "auto" => ChatToolChoice::Auto,
                "required" => ChatToolChoice::Required,
                _ => ChatToolChoice::Function(FunctionName { name: s }),
            },
            ToolChoiceRepr::TypedChoice(TypedChoice::Function { function }) => {
                ChatToolChoice::Function(function)
            }
        }
    }
}

#[derive(Clone, Deserialize, Default)]
#[serde(from = "ToolChoiceRepr")]
pub enum ChatToolChoice {
    #[default]
    Auto,
    None,
    Required,
    Function(FunctionName),
}
