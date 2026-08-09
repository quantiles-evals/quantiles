use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::environment::ToolEnvironment;
use super::model::{ModelMessage, ToolChatModel, ToolResult};

const END_TOKEN: &str = "<END>";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Trajectory {
    pub(crate) messages: Vec<TrajectoryMessage>,
    pub(crate) tool_calls: usize,
    pub(crate) turns: usize,
    pub(crate) terminated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", rename_all = "snake_case")]
pub(crate) enum TrajectoryMessage {
    User {
        content: String,
    },
    Assistant {
        content: String,
    },
    Tool {
        name: String,
        arguments: serde_json::Value,
        result: serde_json::Value,
    },
}

pub(crate) struct RunConversation<'a> {
    pub(crate) agent: &'a dyn ToolChatModel,
    pub(crate) user: &'a dyn ToolChatModel,
    pub(crate) environment: &'a mut dyn ToolEnvironment,
    pub(crate) agent_policy: &'a str,
    pub(crate) user_goal: &'a str,
    pub(crate) max_turns: usize,
}

pub(crate) async fn run_conversation(args: RunConversation<'_>) -> Result<Trajectory> {
    let mut trajectory = Trajectory {
        messages: Vec::new(),
        tool_calls: 0,
        turns: 0,
        terminated: false,
    };
    let mut agent_messages = Vec::new();

    for turn in 0..args.max_turns {
        let user_text = generate_user_message(args.user, args.user_goal, &trajectory, turn).await?;
        if user_text.trim() == END_TOKEN {
            trajectory.terminated = true;
            break;
        }
        trajectory.messages.push(TrajectoryMessage::User {
            content: user_text.clone(),
        });
        agent_messages.push(ModelMessage::User { content: user_text });

        loop {
            let response = args
                .agent
                .generate(
                    args.agent_policy,
                    &agent_messages,
                    &args.environment.tools(),
                )
                .await
                .context("agent generation failed")?;
            if response.content.is_none() && response.tool_calls.is_empty() {
                bail!("agent returned neither text nor a tool call");
            }
            agent_messages.push(ModelMessage::Assistant {
                content: response.content.clone(),
                tool_calls: response.tool_calls.clone(),
            });

            if !response.tool_calls.is_empty() {
                let mut results = Vec::with_capacity(response.tool_calls.len());
                for call in &response.tool_calls {
                    let result =
                        args.environment.invoke(call).await.unwrap_or_else(
                            |error| serde_json::json!({"error": format!("{error:#}")}),
                        );
                    trajectory.messages.push(TrajectoryMessage::Tool {
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                        result: result.clone(),
                    });
                    results.push(ToolResult {
                        call_id: call.call_id.clone(),
                        name: call.name.clone(),
                        content: serde_json::to_string(&result)
                            .expect("JSON value is serializable"),
                    });
                    trajectory.tool_calls += 1;
                }
                agent_messages.push(ModelMessage::Tool { results });
                if args.environment.terminated() {
                    trajectory.terminated = true;
                    break;
                }
                continue;
            }

            if let Some(content) = response.content {
                trajectory
                    .messages
                    .push(TrajectoryMessage::Assistant { content });
            }
            break;
        }
        trajectory.turns = turn + 1;
        if trajectory.terminated {
            break;
        }
    }

    Ok(trajectory)
}

async fn generate_user_message(
    user: &dyn ToolChatModel,
    goal: &str,
    trajectory: &Trajectory,
    turn: usize,
) -> Result<String> {
    let transcript = visible_transcript(trajectory);
    let prompt = if turn == 0 {
        "Begin the conversation as the customer.".to_owned()
    } else {
        format!(
            "Conversation so far:\n{transcript}\n\nRespond as the customer, or output {END_TOKEN} if the request is complete."
        )
    };
    let response = user
        .generate(
            &format!(
                "You simulate a customer for an evaluation. Follow this private goal exactly and do not invent facts:\n{goal}\nOutput only the customer's next message. Output {END_TOKEN} once the goal is satisfied."
            ),
            &[ModelMessage::User { content: prompt }],
            &[],
        )
        .await
        .context("user simulator generation failed")?;
    if !response.tool_calls.is_empty() {
        bail!("user simulator attempted a tool call");
    }
    response.content.context("user simulator returned no text")
}

pub(crate) fn visible_transcript(trajectory: &Trajectory) -> String {
    trajectory
        .messages
        .iter()
        .filter_map(|message| match message {
            TrajectoryMessage::User { content } => Some(format!("Customer: {content}")),
            TrajectoryMessage::Assistant { content } => Some(format!("Agent: {content}")),
            TrajectoryMessage::Tool { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
