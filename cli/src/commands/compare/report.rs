use std::collections::BTreeSet;

use qt::time::format_duration;

use crate::commands::compare::{
    metrics::MetricComparison,
    run::{RunData, RunInfo},
    step::{StepRefs, build_step_rows},
};
use comfy_table::{Cell, ContentArrangement, Table, presets::NOTHING};
use serde::Serialize;
use serde_json::Value;

const BENCHMARK_NAME_WARNING: &str = "benchmark names differ, comparisons may be inaccurate";
const BENCHMARK_NAME_WARNING_DISPLAY: &str =
    "WARNING: benchmark names differ, comparisons may be inaccurate";

#[derive(Serialize)]
pub(super) struct CompareReport {
    run_a: RunInfo,
    run_b: RunInfo,
    pub(super) differs: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    input_comparison: WorkflowFieldComparison,
    output_comparison: WorkflowFieldComparison,
    steps: Vec<StepComparison>,
    output_differences: Vec<OutputDifference>,
    metrics: Vec<MetricComparison>,
}

impl CompareReport {
    pub(super) fn build(run_a: RunData, run_b: RunData) -> Self {
        let (steps, step_refs) = build_step_rows(&run_a.steps, &run_b.steps);
        let output_differences = OutputDifference::from_rows(&step_refs);
        let metrics = MetricComparison::from_summaries(&run_a.metrics, &run_b.metrics);

        let input_same = run_a.run.input == run_b.run.input;
        let output_same = run_a.run.output == run_b.run.output;
        let warning = (run_a.run.workflow_name != run_b.run.workflow_name)
            .then(|| BENCHMARK_NAME_WARNING.to_owned());

        let differs = steps
            .iter()
            .any(|row| row.input.differs() || row.status.differs() || row.output.differs())
            || metrics.iter().any(|metric| !metric.same)
            || !input_same
            || !output_same;

        Self {
            run_a: RunInfo {
                id: run_a.run.id,
                workflow_name: run_a.run.workflow_name,
                model_name: run_a.run.model_name,
                status: run_a.run.status,
                duration: run_a.run.finished_at.map(|f| f - run_a.run.started_at),
            },
            run_b: RunInfo {
                id: run_b.run.id,
                workflow_name: run_b.run.workflow_name,
                model_name: run_b.run.model_name,
                status: run_b.run.status,
                duration: run_b.run.finished_at.map(|f| f - run_b.run.started_at),
            },
            warning,
            input_comparison: WorkflowFieldComparison {
                same: input_same,
                run_a: run_a.run.input,
                run_b: run_b.run.input,
            },
            output_comparison: WorkflowFieldComparison {
                same: output_same,
                run_a: run_a.run.output,
                run_b: run_b.run.output,
            },
            differs,
            steps,
            output_differences,
            metrics,
        }
    }

    pub(super) fn print(&self) {
        let table = self.new_table();
        let table = self.print_summary(table);
        let table = print_metrics(table, &self.metrics);
        println!("Comparing runs {} and {}", self.run_a.id, self.run_b.id);
        println!("{table}");
    }

    fn new_table(&self) -> Table {
        let mut table = Table::new();
        table
            .load_preset(NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                String::new(),
                format!("Run {}", &self.run_a.id).to_string(),
                format!("Run {}", &self.run_b.id).to_string(),
                "Delta".to_string(),
            ]);
        table
    }

    #[expect(
        clippy::similar_names,
        reason = "paired run names keep comparison columns clear"
    )]
    fn print_summary(&self, mut table: Table) -> Table {
        let eval_name_delta = if self.run_a.workflow_name == self.run_b.workflow_name {
            "SAME"
        } else {
            BENCHMARK_NAME_WARNING_DISPLAY
        };
        table.add_row(vec![
            Cell::new("Eval"),
            Cell::new(self.run_a.workflow_name.clone()),
            Cell::new(self.run_b.workflow_name.clone()),
            // delta
            Cell::new(eval_name_delta),
        ]);
        table.add_row(vec![
            Cell::new("Status"),
            Cell::new(self.run_a.completed_marker()),
            Cell::new(self.run_b.completed_marker()),
            Cell::new(""),
        ]);

        let duration_delta = match (self.run_a.duration, self.run_b.duration) {
            (Some(run_a_dur), Some(run_b_dur)) => format_duration(run_b_dur - run_a_dur),
            _ => "-".to_string(),
        };
        table.add_row(vec![
            Cell::new("Duration"),
            Cell::new(self.run_a.duration.map_or("-".to_string(), format_duration)),
            Cell::new(self.run_b.duration.map_or("-".to_string(), format_duration)),
            Cell::new(duration_delta),
        ]);

        let (run_a_model_name, run_b_model_name) = (
            self.run_a
                .model_name
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            self.run_b
                .model_name
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
        let model_name_delta = if run_a_model_name == run_b_model_name {
            "SAME"
        } else {
            "DIFFERENT"
        };
        table.add_row(vec![
            Cell::new("Model"),
            Cell::new(run_a_model_name),
            Cell::new(run_b_model_name),
            Cell::new(model_name_delta),
        ]);

        table
    }
}

#[derive(Serialize)]
pub(super) struct StepComparison {
    pub(super) step: String,
    pub(super) present: Presence,
    pub(super) input: ComparisonState,
    pub(super) status: ComparisonState,
    pub(super) output: ComparisonState,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Presence {
    Both,
    OnlyA,
    OnlyB,
    Neither,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ComparisonState {
    Same,
    Differs,
}

impl ComparisonState {
    pub(super) const fn from_same(same: bool) -> Self {
        if same { Self::Same } else { Self::Differs }
    }

    pub(super) const fn differs(&self) -> bool {
        matches!(self, Self::Differs)
    }
}

#[derive(Serialize)]
pub(super) struct OutputDifference {
    pub(super) step: String,
    pub(super) run_a: OutputValue,
    pub(super) run_b: OutputValue,
    pub(super) field_differences: Vec<JsonFieldDifference>,
}

impl OutputDifference {
    pub(super) fn from_rows(rows: &[StepRefs<'_>]) -> Vec<OutputDifference> {
        rows.iter()
            .filter_map(|row| {
                let output_a = row.step_a?.output.as_deref().unwrap_or("(none)");
                let output_b = row.step_b?.output.as_deref().unwrap_or("(none)");
                if output_a == output_b {
                    return None;
                }

                let run_a = OutputValue::from_raw(output_a);
                let run_b = OutputValue::from_raw(output_b);
                let field_differences = match (&run_a.json, &run_b.json) {
                    (Some(a), Some(b)) => json_field_differences(a, b),
                    _ => Vec::new(),
                };

                Some(OutputDifference {
                    step: row.key.clone(),
                    run_a,
                    run_b,
                    field_differences,
                })
            })
            .collect()
    }
}

#[derive(Serialize)]
pub(super) struct OutputValue {
    pub(super) raw: String,
    pub(super) json: Option<Value>,
}

impl OutputValue {
    pub(super) fn from_raw(raw: &str) -> Self {
        Self {
            raw: raw.to_owned(),
            json: serde_json::from_str(raw).ok(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct JsonFieldDifference {
    pub(super) path: String,
    pub(super) run_a: Option<Value>,
    pub(super) run_b: Option<Value>,
}

fn print_metrics(mut table: Table, metrics: &[MetricComparison]) -> Table {
    if metrics.is_empty() {
        return table;
    }

    for metric in metrics {
        table.add_row(vec![
            Cell::new(&metric.name),
            metric.run_a.to_cell(),
            metric.run_b.to_cell(),
            metric.deltas.to_cell(),
        ]);
    }

    table
}

#[derive(Serialize)]
struct WorkflowFieldComparison {
    same: bool,
    run_a: Option<String>,
    run_b: Option<String>,
}

fn json_field_differences(left: &Value, right: &Value) -> Vec<JsonFieldDifference> {
    let (Some(left_obj), Some(right_obj)) = (left.as_object(), right.as_object()) else {
        if left == right {
            return Vec::new();
        }
        return vec![JsonFieldDifference {
            path: "$".to_owned(),
            run_a: Some(left.clone()),
            run_b: Some(right.clone()),
        }];
    };

    let mut keys = BTreeSet::new();
    keys.extend(left_obj.keys());
    keys.extend(right_obj.keys());

    keys.into_iter()
        .filter_map(|key| {
            let left_value = left_obj.get(key);
            let right_value = right_obj.get(key);
            if left_value == right_value {
                return None;
            }
            Some(JsonFieldDifference {
                path: format!("$.{key}"),
                run_a: left_value.cloned(),
                run_b: right_value.cloned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use qt::db::{RunStatus, WorkflowRun};
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;

    #[test]
    fn json_includes_warning_for_different_benchmark_names() {
        let report = CompareReport::build(run_data(1, "alpha"), run_data(2, "beta"));

        let json = serde_json::to_value(report).expect("report should serialize");

        assert_eq!(
            json["warning"],
            json!("benchmark names differ, comparisons may be inaccurate")
        );
    }

    #[test]
    fn json_omits_warning_for_same_benchmark_names() {
        let report = CompareReport::build(run_data(1, "alpha"), run_data(2, "alpha"));

        let json = serde_json::to_value(report).expect("report should serialize");

        assert!(json.get("warning").is_none());
    }

    fn run_data(id: i64, workflow_name: &str) -> RunData {
        let now = OffsetDateTime::now_utc();
        RunData {
            run: WorkflowRun {
                id,
                workflow_name: workflow_name.to_owned(),
                model_name: None,
                status: RunStatus::Completed,
                input: None,
                output: None,
                started_at: now,
                finished_at: Some(now),
                error: None,
            },
            steps: Vec::new(),
            metrics: Vec::new(),
        }
    }
}
