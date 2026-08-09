use anyhow::{Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::model::{ToolDefinition, ToolInvocation};

#[async_trait]
pub(crate) trait ToolEnvironment: Send {
    fn tools(&self) -> Vec<ToolDefinition>;
    async fn invoke(&mut self, call: &ToolInvocation) -> Result<Value>;
    fn state(&self) -> Value;
    fn terminated(&self) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct MockState {
    pub(crate) customer_id: String,
    pub(crate) name: String,
    pub(crate) email: String,
}

pub(crate) struct MockEnvironment {
    state: MockState,
    terminated: bool,
}

impl MockEnvironment {
    pub(crate) fn new(state: MockState) -> Self {
        Self {
            state,
            terminated: false,
        }
    }
}

#[async_trait]
impl ToolEnvironment for MockEnvironment {
    fn tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "get_customer".to_owned(),
                description: "Look up a customer record by customer ID.".to_owned(),
                schema: json!({
                    "type": "object",
                    "properties": {"customer_id": {"type": "string"}},
                    "required": ["customer_id"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "update_customer_email".to_owned(),
                description: "Update the email address on a customer record.".to_owned(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "customer_id": {"type": "string"},
                        "email": {"type": "string"}
                    },
                    "required": ["customer_id", "email"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "transfer_to_human".to_owned(),
                description: "End the interaction and transfer the customer to a human agent."
                    .to_owned(),
                schema: json!({
                    "type": "object",
                    "properties": {"reason": {"type": "string"}},
                    "required": ["reason"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    async fn invoke(&mut self, call: &ToolInvocation) -> Result<Value> {
        match call.name.as_str() {
            "get_customer" => {
                require_customer_id(&call.arguments, &self.state.customer_id)?;
                Ok(serde_json::to_value(&self.state)?)
            }
            "update_customer_email" => {
                require_customer_id(&call.arguments, &self.state.customer_id)?;
                let email = required_string(&call.arguments, "email")?;
                if !email.contains('@') {
                    bail!("email must contain @");
                }
                email.clone_into(&mut self.state.email);
                Ok(json!({"status": "success", "customer": self.state}))
            }
            "transfer_to_human" => {
                let _ = required_string(&call.arguments, "reason")?;
                self.terminated = true;
                Ok(json!({"status": "transferred"}))
            }
            other => bail!("unknown tool `{other}`"),
        }
    }

    fn state(&self) -> Value {
        serde_json::to_value(&self.state).expect("MockState is serializable")
    }

    fn terminated(&self) -> bool {
        self.terminated
    }
}

fn require_customer_id(arguments: &Value, expected: &str) -> Result<()> {
    let actual = required_string(arguments, "customer_id")?;
    if actual != expected {
        bail!("customer `{actual}` not found");
    }
    Ok(())
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing or invalid `{key}` argument"))
}
