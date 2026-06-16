use std::collections::HashMap;

use qt::db::StepSummary;

use crate::commands::compare::report::{ComparisonState, Presence, StepComparison};

pub(super) struct StepRefs<'a> {
    pub(super) key: String,
    pub(super) step_a: Option<&'a StepSummary>,
    pub(super) step_b: Option<&'a StepSummary>,
}

/// Build user-facing step comparisons and borrowed step refs for detailed diffs.
pub(super) fn build_step_rows<'a>(
    left_steps: &'a [StepSummary],
    right_steps: &'a [StepSummary],
) -> (Vec<StepComparison>, Vec<StepRefs<'a>>) {
    let map_a: HashMap<&str, &StepSummary> = left_steps
        .iter()
        .map(|s| (s.step_key.as_str(), s))
        .collect();
    let map_b: HashMap<&str, &StepSummary> = right_steps
        .iter()
        .map(|s| (s.step_key.as_str(), s))
        .collect();

    let mut keys: Vec<&str> = map_a.keys().copied().collect();
    keys.extend(map_b.keys().copied());
    keys.sort_unstable();
    keys.dedup();

    let mut rows = Vec::new();
    let mut refs = Vec::new();

    for key in keys {
        let step_a = map_a.get(key).copied();
        let step_b = map_b.get(key).copied();

        let input_same = match (step_a, step_b) {
            (Some(a), Some(b)) => a.input_hash == b.input_hash,
            _ => false,
        };
        let status_same = match (step_a, step_b) {
            (Some(a), Some(b)) => a.status == b.status,
            _ => false,
        };
        let output_same = match (step_a, step_b) {
            (Some(a), Some(b)) => a.output == b.output,
            _ => false,
        };

        let present = match (step_a.is_some(), step_b.is_some()) {
            (true, true) => Presence::Both,
            (true, false) => Presence::OnlyA,
            (false, true) => Presence::OnlyB,
            (false, false) => Presence::Neither,
        };

        rows.push(StepComparison {
            step: key.to_owned(),
            present,
            input: ComparisonState::from_same(input_same),
            status: ComparisonState::from_same(status_same),
            output: ComparisonState::from_same(output_same),
        });

        refs.push(StepRefs {
            key: key.to_owned(),
            step_a,
            step_b,
        });
    }

    (rows, refs)
}
