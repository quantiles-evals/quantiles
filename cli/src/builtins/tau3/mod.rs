use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::builtins::common::{hash_input, run_timed_step};
use crate::builtins::{BuiltinContext, BuiltinWorkflow};
use crate::llm::Sampler;

use environment::{MockEnvironment, MockState, ToolEnvironment};
use model::{GenaiToolChatModel, Tau3DemoToolChatModel, ToolChatModel};
use orchestrator::{RunConversation, Trajectory, run_conversation, visible_transcript};
use scoring::{Scores, pass_at_k, score};

mod environment;
mod model;
mod orchestrator;
mod scoring;

const HARNESS_VERSION: &str = "tau3-native-conformance-v1";
const DEFAULT_AGENT_POLICY: &str = "You are a customer-service agent. Follow policy, use tools to inspect or change records, never claim an action succeeded before a tool confirms it, and clearly communicate the outcome to the customer.";

/// Native structured-tool-calling conformance benchmark for the Quantiles τ³ harness.
pub struct Tau3MockBuiltin;

/// Reserved native benchmark name for the forthcoming official airline domain.
pub struct Tau3AirlineBuiltin;

#[async_trait::async_trait]
impl BuiltinWorkflow for Tau3AirlineBuiltin {
    fn name(&self) -> String {
        "tau3-airline".to_owned()
    }

    async fn execute(&self, _ctx: BuiltinContext<'_>) -> Result<()> {
        bail!("tau3-airline is recognized but not implemented yet")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tau3Config {
    model: Option<Sampler>,
    #[serde(default)]
    limit: Option<usize>,
    user_model: Option<Sampler>,
    #[serde(default = "default_trials")]
    trials: usize,
    #[serde(default = "default_max_turns")]
    max_turns: usize,
}

impl Default for Tau3Config {
    fn default() -> Self {
        Self {
            model: None,
            limit: None,
            user_model: None,
            trials: default_trials(),
            max_turns: default_max_turns(),
        }
    }
}

const fn default_trials() -> usize {
    1
}
const fn default_max_turns() -> usize {
    20
}

#[derive(Clone)]
struct Task {
    id: &'static str,
    user_goal: &'static str,
    initial_state: MockState,
    expected_state: Value,
    required_communication: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TrialOutput {
    task_id: String,
    trial: usize,
    trajectory: Trajectory,
    final_state: Value,
    reward: Scores,
}

#[derive(Serialize)]
struct RunInput<'a> {
    harness_version: &'a str,
    domain: &'a str,
    agent_model: String,
    user_model: String,
    tasks: usize,
    trials: usize,
    max_turns: usize,
}

#[derive(Serialize)]
struct RunOutput {
    harness_version: &'static str,
    domain: &'static str,
    tasks_completed: usize,
    trials_completed: usize,
}

struct ResolvedModels {
    agent: Arc<dyn ToolChatModel>,
    user: Arc<dyn ToolChatModel>,
    agent_name: String,
    user_name: String,
}

fn resolve_models(config: &Tau3Config) -> Result<ResolvedModels> {
    let (agent, agent_name): (Arc<dyn ToolChatModel>, String) = match &config.model {
        Some(sampler) => (
            Arc::new(GenaiToolChatModel::from_sampler(sampler)?),
            sampler.to_string(),
        ),
        None => (
            Arc::new(Tau3DemoToolChatModel::agent()),
            Tau3DemoToolChatModel::NAME.to_owned(),
        ),
    };
    let (user, user_name): (Arc<dyn ToolChatModel>, String) = match &config.user_model {
        Some(sampler) => (
            Arc::new(GenaiToolChatModel::from_sampler(sampler)?),
            sampler.to_string(),
        ),
        None if config.model.is_some() => (
            Arc::new(GenaiToolChatModel::from_sampler(
                config.model.as_ref().expect("model is present"),
            )?),
            agent_name.clone(),
        ),
        None => (
            Arc::new(Tau3DemoToolChatModel::user()),
            Tau3DemoToolChatModel::NAME.to_owned(),
        ),
    };
    Ok(ResolvedModels {
        agent,
        user,
        agent_name,
        user_name,
    })
}

#[async_trait::async_trait]
impl BuiltinWorkflow for Tau3MockBuiltin {
    fn name(&self) -> String {
        "tau3-mock".to_owned()
    }

    #[expect(clippy::cast_precision_loss)]
    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()> {
        let config: Tau3Config = ctx
            .input
            .map(serde_json::from_str)
            .transpose()
            .context("invalid tau3-mock input JSON")?
            .unwrap_or_default();
        if config.trials == 0 {
            bail!("trials must be > 0");
        }
        if config.max_turns == 0 {
            bail!("max_turns must be > 0");
        }
        if config.limit == Some(0) {
            bail!("limit must be > 0");
        }

        let models = resolve_models(&config)?;
        let mut tasks = tasks();
        if let Some(limit) = config.limit {
            tasks.truncate(limit.min(tasks.len()));
        }

        let run_input = RunInput {
            harness_version: HARNESS_VERSION,
            domain: "mock",
            agent_model: models.agent_name.clone(),
            user_model: models.user_name.clone(),
            tasks: tasks.len(),
            trials: config.trials,
            max_turns: config.max_turns,
        };
        crate::db::set_run_input(ctx.db, ctx.run_id, &serde_json::to_string(&run_input)?).await?;

        let mut rewards_by_task = Vec::with_capacity(tasks.len());
        let mut trials_completed = 0usize;
        for task in tasks {
            let mut task_rewards = Vec::with_capacity(config.trials);
            for trial in 0..config.trials {
                let step_key = format!("task-{}-trial-{trial}", task.id);
                let input_hash = hash_input(&format!(
                    "{HARNESS_VERSION}\ntask={}\nagent={}\nuser={}\nmax_turns={}",
                    task.id, models.agent_name, models.user_name, config.max_turns
                ));
                let agent = Arc::clone(&models.agent);
                let user = Arc::clone(&models.user);
                let task = task.clone();
                let (output, step_id) = run_timed_step(
                    ctx.db,
                    ctx.metrics_store,
                    ctx.run_id,
                    &step_key,
                    &input_hash,
                    async move {
                        run_trial(
                            &task,
                            trial,
                            agent.as_ref(),
                            user.as_ref(),
                            config.max_turns,
                        )
                        .await
                    },
                )
                .await?;
                if let Some(step_id) = step_id {
                    for (name, value) in [
                        ("reward", output.reward.overall),
                        ("database_reward", output.reward.database),
                        ("communicate_reward", output.reward.communication),
                        ("tool_calls", output.trajectory.tool_calls as f64),
                        ("turns", output.trajectory.turns as f64),
                    ] {
                        ctx.metrics_store
                            .emit(ctx.run_id, Some(step_id), name, value, None)
                            .await;
                    }
                }
                task_rewards.push(output.reward.overall);
                trials_completed += 1;
            }
            rewards_by_task.push(task_rewards);
        }

        emit_aggregate_metrics(&ctx, &rewards_by_task).await;
        crate::db::set_run_output(
            ctx.db,
            ctx.run_id,
            &serde_json::to_string(&RunOutput {
                harness_version: HARNESS_VERSION,
                domain: "mock",
                tasks_completed: rewards_by_task.len(),
                trials_completed,
            })?,
        )
        .await?;
        Ok(())
    }
}

async fn run_trial(
    task: &Task,
    trial: usize,
    agent: &dyn model::ToolChatModel,
    user: &dyn model::ToolChatModel,
    max_turns: usize,
) -> Result<TrialOutput> {
    let mut environment = MockEnvironment::new(task.initial_state.clone());
    let trajectory = run_conversation(RunConversation {
        agent,
        user,
        environment: &mut environment,
        agent_policy: DEFAULT_AGENT_POLICY,
        user_goal: task.user_goal,
        max_turns,
    })
    .await?;
    let final_state = environment.state();
    let reward = score(
        &final_state,
        &task.expected_state,
        &visible_transcript(&trajectory),
        &task.required_communication,
    );
    Ok(TrialOutput {
        task_id: task.id.to_owned(),
        trial,
        trajectory,
        final_state,
        reward,
    })
}

#[expect(clippy::cast_precision_loss)]
async fn emit_aggregate_metrics(ctx: &BuiltinContext<'_>, rewards_by_task: &[Vec<f64>]) {
    let rewards = rewards_by_task
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    if !rewards.is_empty() {
        let mean = rewards.iter().sum::<f64>() / rewards.len() as f64;
        ctx.metrics_store
            .emit(ctx.run_id, None, "reward", mean, None)
            .await;
    }
    let trials = rewards_by_task.first().map_or(0, Vec::len);
    for k in 1..=trials {
        if let Some(value) = pass_at_k(rewards_by_task, k) {
            ctx.metrics_store
                .emit(ctx.run_id, None, &format!("pass_at_{k}"), value, None)
                .await;
        }
    }
    ctx.metrics_store
        .emit(
            ctx.run_id,
            None,
            "task_count",
            rewards_by_task.len() as f64,
            None,
        )
        .await;
    ctx.metrics_store
        .emit(ctx.run_id, None, "trial_count", rewards.len() as f64, None)
        .await;
}

fn tasks() -> Vec<Task> {
    vec![Task {
        id: "mock-update-email",
        user_goal: "You are Morgan Lee, customer ID C-100. Ask the agent to change your account email from old@example.com to morgan.lee@example.com. Do not provide information the agent has not requested.",
        initial_state: MockState {
            customer_id: "C-100".to_owned(),
            name: "Morgan Lee".to_owned(),
            email: "old@example.com".to_owned(),
        },
        expected_state: json!({"customer_id": "C-100", "email": "morgan.lee@example.com"}),
        required_communication: vec!["morgan.lee@example.com".to_owned()],
    }]
}

#[cfg(test)]
mod tests {
    use super::model::{ModelMessage, ModelTurn, ToolChatModel, ToolDefinition, ToolInvocation};
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct ScriptedModel(Mutex<VecDeque<ModelTurn>>);

    impl ScriptedModel {
        fn new(turns: Vec<ModelTurn>) -> Self {
            Self(Mutex::new(turns.into()))
        }
    }

    #[async_trait]
    impl ToolChatModel for ScriptedModel {
        async fn generate(
            &self,
            _: &str,
            _: &[ModelMessage],
            _: &[ToolDefinition],
        ) -> Result<ModelTurn> {
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("script exhausted"))
        }
    }

    #[tokio::test]
    async fn native_tool_loop_updates_state_and_scores_trajectory() {
        let user = ScriptedModel::new(vec![
            ModelTurn {
                content: Some("Please change my email. My customer ID is C-100.".to_owned()),
                tool_calls: vec![],
            },
            ModelTurn {
                content: Some("<END>".to_owned()),
                tool_calls: vec![],
            },
        ]);
        let agent = ScriptedModel::new(vec![
            ModelTurn {
                content: None,
                tool_calls: vec![ToolInvocation {
                    call_id: "1".to_owned(),
                    name: "get_customer".to_owned(),
                    arguments: json!({"customer_id": "C-100"}),
                    thought_signatures: None,
                }],
            },
            ModelTurn {
                content: None,
                tool_calls: vec![ToolInvocation {
                    call_id: "2".to_owned(),
                    name: "update_customer_email".to_owned(),
                    arguments: json!({"customer_id": "C-100", "email": "morgan.lee@example.com"}),
                    thought_signatures: None,
                }],
            },
            ModelTurn {
                content: Some("Your email is now morgan.lee@example.com.".to_owned()),
                tool_calls: vec![],
            },
        ]);
        let output = run_trial(&tasks()[0], 0, &agent, &user, 5).await.unwrap();
        assert!((output.reward.overall - 1.0).abs() < f64::EPSILON);
        assert_eq!(output.trajectory.tool_calls, 2);
        assert_eq!(output.final_state["email"], "morgan.lee@example.com");
    }

    #[test]
    fn rejects_sampler_without_structured_tools() {
        let result = GenaiToolChatModel::from_sampler(&Sampler::Random);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_exposes_explicit_tau3_names() {
        assert!(crate::builtins::resolve("tau3-mock").is_some());
        assert!(crate::builtins::resolve("tau3-airline").is_some());
        assert!(crate::builtins::resolve("tau3").is_none());
    }

    #[tokio::test]
    async fn airline_placeholder_returns_not_implemented_error() {
        let tmpdir = tempfile::tempdir().unwrap();
        crate::db::init_workspace(tmpdir.path()).await.unwrap();
        let db = crate::db::open_workspace(tmpdir.path()).await.unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(crate::db::metrics_dir(tmpdir.path())).unwrap();
        let run_id = crate::db::create_run(&db, "tau3-airline", None)
            .await
            .unwrap();
        let builtin = crate::builtins::resolve("tau3-airline").unwrap();

        let error = builtin
            .execute(BuiltinContext {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name: "tau3-airline",
                input: None,
                quiet: true,
            })
            .await
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "tau3-airline is recognized but not implemented yet"
        );
    }

    #[tokio::test]
    async fn mock_defaults_to_local_structured_tool_demo_models() {
        let tmpdir = tempfile::tempdir().unwrap();
        crate::db::init_workspace(tmpdir.path()).await.unwrap();
        let db = crate::db::open_workspace(tmpdir.path()).await.unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(crate::db::metrics_dir(tmpdir.path())).unwrap();
        let run_id = crate::db::create_run(&db, "tau3-mock", None).await.unwrap();
        let builtin = crate::builtins::resolve("tau3-mock").unwrap();

        builtin
            .execute(BuiltinContext {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name: "tau3-mock",
                input: None,
                quiet: true,
            })
            .await
            .unwrap();

        let run = crate::db::get_run(&db, run_id).await.unwrap();
        let input: Value = serde_json::from_str(run.input.as_deref().unwrap()).unwrap();
        let output: Value = serde_json::from_str(run.output.as_deref().unwrap()).unwrap();
        assert_eq!(input["agent_model"], Tau3DemoToolChatModel::NAME);
        assert_eq!(input["user_model"], Tau3DemoToolChatModel::NAME);
        assert_eq!(output["tasks_completed"], 1);
        assert_eq!(output["trials_completed"], 1);
    }
}
