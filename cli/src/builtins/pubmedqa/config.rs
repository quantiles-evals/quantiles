use serde::{Deserialize, Serialize};

use crate::builtins::common::BuiltinConfig;

/// Parsed user input for the builtin.
///
/// All common fields (`limit`, `model`, `max_workers`) live in [`BuiltinConfig`]
/// and are flattened during deserialization so that the TOML surface stays flat.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PubMedQAConfig {
    #[serde(flatten)]
    pub(crate) base: BuiltinConfig,
}

/// Per-row step output stored as JSON in the step record.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RowOutput {
    pub(crate) sample_id: String,
    pub(crate) question: String,
    pub(crate) context: String,
    pub(crate) gold_answer: String,
    pub(crate) prediction: Option<String>,
    pub(crate) is_correct: bool,
    pub(crate) model_response: String,
}
