use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChatImageUrl {
    pub url: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChatUserChunk {
    Text { text: String },
    ImageUrl { image_url: ChatImageUrl },
    // InputAudio { input_audio: }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChatAssistantChunk {
    Text { text: String },
    Refusal { refusal: String },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageContent<T> {
    SingleText(String),
    ManyChunks(Vec<T>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallType {
    Function,
}

#[derive(Serialize, Deserialize)]
pub struct ChatFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ChatToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    pub r#type: Option<ToolCallType>,
    pub function: ChatFunction,
}

#[derive(Serialize, Deserialize)]
pub struct ChatSystemDeveloperMessage {
    pub content: ChatMessageContent<String>,
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ChatUserMessage {
    pub content: ChatMessageContent<ChatUserChunk>,
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ChatAssistantMessage {
    pub content: ChatMessageContent<ChatAssistantChunk>,
    pub refusal: Option<String>,
    pub name: Option<String>,
    // pub audio: Option<Vec<u8>>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Serialize, Deserialize)]
pub struct ChatToolMessage {
    pub content: ChatMessageContent<String>,
    pub tool_call_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessageVariant {
    Developer(ChatSystemDeveloperMessage),
    System(ChatSystemDeveloperMessage),
    User(ChatUserMessage),
    Assistant(ChatAssistantMessage),
    Tool(ChatToolMessage),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessages {
    Text(String),
    Array(Vec<ChatMessageVariant>),
}

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    pub content: Vec<ChatMessageChunk>,
    pub role: String,
    pub name: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageChunk {
    Text { text: String },
    Image { image: ChatImageUrl },
    // InputAudio { input_audio: }
    Refusal { refusal: String },
}

impl From<ChatMessages> for Vec<ChatMessage> {
    fn from(value: ChatMessages) -> Self {
        let variants = match value {
            ChatMessages::Text(s) => {
                return vec![ChatMessage {
                    content: vec![ChatMessageChunk::Text { text: s }],
                    role: "user".to_string(),
                    name: None,
                    refusal: None,
                    tool_calls: None,
                    tool_call_id: None,
                }];
            }
            ChatMessages::Array(messages) => messages,
        };

        variants
            .into_iter()
            .map(|m| match m {
                ChatMessageVariant::Developer(msg) => {
                    let content = match msg.content {
                        ChatMessageContent::SingleText(text) => {
                            vec![ChatMessageChunk::Text { text }]
                        }
                        ChatMessageContent::ManyChunks(chunks) => chunks
                            .into_iter()
                            .map(|chunk| ChatMessageChunk::Text { text: chunk })
                            .collect(),
                    };

                    ChatMessage {
                        content,
                        role: "developer".to_string(),
                        name: msg.name,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: None,
                    }
                }
                ChatMessageVariant::System(msg) => {
                    let content = match msg.content {
                        ChatMessageContent::SingleText(text) => {
                            vec![ChatMessageChunk::Text { text }]
                        }
                        ChatMessageContent::ManyChunks(chunks) => chunks
                            .into_iter()
                            .map(|chunk| ChatMessageChunk::Text { text: chunk })
                            .collect(),
                    };

                    ChatMessage {
                        content,
                        role: "system".to_string(),
                        name: msg.name,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: None,
                    }
                }
                ChatMessageVariant::User(msg) => {
                    let content = match msg.content {
                        ChatMessageContent::SingleText(text) => {
                            vec![ChatMessageChunk::Text { text }]
                        }
                        ChatMessageContent::ManyChunks(chunks) => chunks
                            .into_iter()
                            .map(|chunk| match chunk {
                                ChatUserChunk::Text { text } => ChatMessageChunk::Text { text },
                                ChatUserChunk::ImageUrl { image_url } => {
                                    ChatMessageChunk::Image { image: image_url }
                                }
                            })
                            .collect(),
                    };

                    ChatMessage {
                        content,
                        role: "user".to_string(),
                        name: msg.name,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: None,
                    }
                }
                ChatMessageVariant::Assistant(msg) => {
                    let content = match msg.content {
                        ChatMessageContent::SingleText(text) => {
                            vec![ChatMessageChunk::Text { text }]
                        }
                        ChatMessageContent::ManyChunks(chunks) => chunks
                            .into_iter()
                            .map(|chunk| match chunk {
                                ChatAssistantChunk::Text { text } => {
                                    ChatMessageChunk::Text { text }
                                }
                                ChatAssistantChunk::Refusal { refusal } => {
                                    ChatMessageChunk::Refusal { refusal }
                                }
                            })
                            .collect(),
                    };

                    ChatMessage {
                        content,
                        role: "assistant".to_string(),
                        name: msg.name,
                        refusal: msg.refusal,
                        tool_calls: msg.tool_calls,
                        tool_call_id: None,
                    }
                }
                ChatMessageVariant::Tool(msg) => {
                    let content = match msg.content {
                        ChatMessageContent::SingleText(text) => {
                            vec![ChatMessageChunk::Text { text }]
                        }
                        ChatMessageContent::ManyChunks(chunks) => chunks
                            .into_iter()
                            .map(|chunk| ChatMessageChunk::Text { text: chunk })
                            .collect(),
                    };

                    ChatMessage {
                        content,
                        role: "tool".to_string(),
                        name: None,
                        refusal: None,
                        tool_calls: None,
                        tool_call_id: Some(msg.tool_call_id),
                    }
                }
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChatJsonSchema {
    pub description: Option<String>,
    pub name: String,
    pub schema: serde_json::Value,
    pub strict: Option<bool>,
}
