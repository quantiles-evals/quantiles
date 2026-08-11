use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use genai::chat::{
    ChatMessage, ChatRequest, ContentPart, MessageContent, Tool, ToolCall, ToolResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm::Sampler;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ToolDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ToolInvocation {
    pub(crate) call_id: String,
    pub(crate) name: String,
    pub(crate) arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thought_signatures: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ToolResult {
    pub(crate) call_id: String,
    pub(crate) name: String,
    pub(crate) content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", rename_all = "snake_case")]
pub(crate) enum ModelMessage {
    User {
        content: String,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolInvocation>,
    },
    Tool {
        results: Vec<ToolResult>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ModelTurn {
    pub(crate) content: Option<String>,
    pub(crate) tool_calls: Vec<ToolInvocation>,
}

#[async_trait]
pub(crate) trait ToolChatModel: Send + Sync {
    async fn generate(
        &self,
        system: &str,
        messages: &[ModelMessage],
        tools: &[ToolDefinition],
    ) -> Result<ModelTurn>;
}

/// Local deterministic model used to validate the bundled mock harness without
/// making provider calls. It intentionally supports only the bundled mock task.
pub(crate) struct Tau3DemoToolChatModel {
    role: DemoRole,
}

enum DemoRole {
    Agent,
    User,
}

impl Tau3DemoToolChatModel {
    pub(crate) const NAME: &'static str = "tau3-demo";

    pub(crate) const fn agent() -> Self {
        Self {
            role: DemoRole::Agent,
        }
    }

    pub(crate) const fn user() -> Self {
        Self {
            role: DemoRole::User,
        }
    }

    fn generate_agent(messages: &[ModelMessage], tools: &[ToolDefinition]) -> Result<ModelTurn> {
        let tool_call = match messages.last() {
            Some(ModelMessage::User { .. }) => ToolInvocation {
                call_id: "tau3-demo-get-customer".to_owned(),
                name: "get_customer".to_owned(),
                arguments: serde_json::json!({"customer_id": "C-100"}),
                thought_signatures: None,
            },
            Some(ModelMessage::Tool { results })
                if results
                    .last()
                    .is_some_and(|result| result.name == "get_customer") =>
            {
                ToolInvocation {
                    call_id: "tau3-demo-update-email".to_owned(),
                    name: "update_customer_email".to_owned(),
                    arguments: serde_json::json!({
                        "customer_id": "C-100",
                        "email": "morgan.lee@example.com"
                    }),
                    thought_signatures: None,
                }
            }
            Some(ModelMessage::Tool { results })
                if results
                    .last()
                    .is_some_and(|result| result.name == "update_customer_email") =>
            {
                return Ok(ModelTurn {
                    content: Some("Your email is now morgan.lee@example.com.".to_owned()),
                    tool_calls: Vec::new(),
                });
            }
            _ => bail!("tau3 demo agent received an unexpected conversation state"),
        };

        if !tools.iter().any(|tool| tool.name == tool_call.name) {
            bail!("tau3 demo agent requires the `{}` tool", tool_call.name);
        }
        Ok(ModelTurn {
            content: None,
            tool_calls: vec![tool_call],
        })
    }

    fn generate_user(messages: &[ModelMessage], tools: &[ToolDefinition]) -> Result<ModelTurn> {
        if !tools.is_empty() {
            bail!("tau3 demo user does not support tools");
        }
        let Some(ModelMessage::User { content: prompt }) = messages.last() else {
            bail!("tau3 demo user received an unexpected conversation state");
        };
        let content = if prompt == "Begin the conversation as the customer." {
            "Please change my email to morgan.lee@example.com. My customer ID is C-100."
        } else {
            "<END>"
        };
        Ok(ModelTurn {
            content: Some(content.to_owned()),
            tool_calls: Vec::new(),
        })
    }
}

#[async_trait]
impl ToolChatModel for Tau3DemoToolChatModel {
    async fn generate(
        &self,
        _system: &str,
        messages: &[ModelMessage],
        tools: &[ToolDefinition],
    ) -> Result<ModelTurn> {
        match self.role {
            DemoRole::Agent => Self::generate_agent(messages, tools),
            DemoRole::User => Self::generate_user(messages, tools),
        }
    }
}

pub(crate) struct GenaiToolChatModel {
    client: genai::Client,
    model: String,
}

impl GenaiToolChatModel {
    pub(crate) fn from_sampler(sampler: &Sampler) -> Result<Self> {
        let model = match sampler {
            Sampler::OpenAI { model_id } => format!("openai::{model_id}"),
            Sampler::Anthropic { model_id } => format!("anthropic::{model_id}"),
            Sampler::Gemini { model_id } => format!("gemini::{model_id}"),
            Sampler::Random | Sampler::RandomLabel | Sampler::CloudflareAIGateway { .. } => {
                bail!(
                    "tau3-mock requires a model backend with structured tool calling; supported providers are openai, anthropic, and gemini"
                )
            }
        };
        Ok(Self {
            client: genai::Client::default(),
            model,
        })
    }
}

#[async_trait]
impl ToolChatModel for GenaiToolChatModel {
    async fn generate(
        &self,
        system: &str,
        messages: &[ModelMessage],
        tools: &[ToolDefinition],
    ) -> Result<ModelTurn> {
        let messages = messages
            .iter()
            .map(to_genai_message)
            .collect::<Result<Vec<_>>>()?;
        let tools = tools
            .iter()
            .map(|tool| {
                Tool::new(tool.name.clone())
                    .with_description(tool.description.clone())
                    .with_schema(tool.schema.clone())
            })
            .collect::<Vec<_>>();
        let mut request = ChatRequest::from_messages(messages).with_system(system);
        if !tools.is_empty() {
            request = request.with_tools(tools);
        }
        let response = self
            .client
            .exec_chat(&self.model, request, None)
            .await
            .with_context(|| format!("{} chat request failed", self.model))?;

        let content = {
            let texts = response.content.texts();
            (!texts.is_empty()).then(|| texts.join("\n"))
        };
        let tool_calls = response
            .content
            .tool_calls()
            .into_iter()
            .map(|call| ToolInvocation {
                call_id: call.call_id.clone(),
                name: call.fn_name.clone(),
                arguments: call.fn_arguments.clone(),
                thought_signatures: call.thought_signatures.clone(),
            })
            .collect();
        Ok(ModelTurn {
            content,
            tool_calls,
        })
    }
}

fn to_genai_message(message: &ModelMessage) -> Result<ChatMessage> {
    Ok(match message {
        ModelMessage::User { content } => ChatMessage::user(content.clone()),
        ModelMessage::Assistant {
            content,
            tool_calls,
        } => {
            let mut parts = Vec::new();
            if let Some(content) = content {
                parts.push(ContentPart::Text(content.clone()));
            }
            for call in tool_calls {
                parts.push(ContentPart::ToolCall(ToolCall {
                    call_id: call.call_id.clone(),
                    fn_name: call.name.clone(),
                    fn_arguments: call.arguments.clone(),
                    thought_signatures: call.thought_signatures.clone(),
                }));
            }
            ChatMessage::assistant(MessageContent::from_parts(parts))
        }
        ModelMessage::Tool { results } => {
            if results.is_empty() {
                bail!("tool message must contain at least one result");
            }
            ChatMessage::from(
                results
                    .iter()
                    .map(|result| {
                        ToolResponse::new(&result.call_id, &result.content)
                            .with_fn_name(&result.name)
                    })
                    .collect::<Vec<_>>(),
            )
        }
    })
}
