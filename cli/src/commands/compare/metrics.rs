use std::collections::HashMap;

use comfy_table::Cell;
use qt::db;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MetricDirection {
    Up,
    Down,
    Mixed,
    Same,
}

impl MetricDirection {
    fn from_deltas(deltas: &MetricDeltas) -> Self {
        let mut has_up = false;
        let mut has_down = false;

        for delta in deltas.0.iter().flatten() {
            if *delta > 0.0 {
                has_up = true;
            } else if *delta < 0.0 {
                has_down = true;
            }
        }

        match (has_up, has_down) {
            (true, true) => Self::Mixed,
            (true, false) => Self::Up,
            (false, true) => Self::Down,
            (false, false) => Self::Same,
        }
    }
}

fn format_metric_number(value: f64, show_sign: bool) -> String {
    let sign = if show_sign && value > 0.0 { "+" } else { "" };
    if value.fract() == 0.0 {
        return format!("{sign}{value:.0}");
    }

    let formatted = if value.abs() < 0.001 {
        format!("{value:.6}")
    } else {
        format!("{value:.4}")
    };

    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    format!("{sign}{trimmed}")
}

#[derive(Serialize)]
pub(super) struct MetricValues(Vec<f64>);

impl MetricValues {
    pub(super) fn new(vals: Vec<f64>) -> Self {
        Self(vals)
    }

    pub(super) fn to_cell(&self) -> Cell {
        let raw = if self.0.is_empty() {
            "-".to_owned()
        } else {
            self.0
                .iter()
                .map(|value| format_metric_number(*value, false))
                .collect::<Vec<_>>()
                .join(", ")
        };
        Cell::new(raw)
    }
}

#[derive(Serialize)]
pub(super) struct MetricDeltas(Vec<Option<f64>>);

impl MetricDeltas {
    pub(super) fn from_left_right(left: &MetricValues, right: &MetricValues) -> Self {
        let max_len = left.0.len().max(right.0.len());
        let raw = (0..max_len)
            .map(|idx| Some(*right.0.get(idx)? - *left.0.get(idx)?))
            .collect();
        Self(raw)
    }

    pub(super) fn to_cell(&self) -> Cell {
        let raw = if self.0.is_empty() {
            "-".to_owned()
        } else {
            self.0
                .iter()
                .map(|delta| match delta {
                    Some(value) => format_metric_number(*value, true),
                    None => "-".to_owned(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        Cell::new(raw)
    }
}

#[derive(Serialize)]
pub(super) struct MetricComparison {
    pub(super) name: String,
    pub(super) run_a: MetricValues,
    pub(super) run_b: MetricValues,
    pub(super) same: bool,
    pub(super) deltas: MetricDeltas,
    pub(super) direction: MetricDirection,
}

impl MetricComparison {
    pub(super) fn from_summaries(
        metrics_a: &[db::MetricPointSummary],
        metrics_b: &[db::MetricPointSummary],
    ) -> Vec<Self> {
        let mut map_a: HashMap<&str, Vec<f64>> = HashMap::new();
        let mut map_b: HashMap<&str, Vec<f64>> = HashMap::new();

        for m in metrics_a {
            map_a
                .entry(m.metric_name.as_str())
                .or_default()
                .push(m.metric_value);
        }
        for m in metrics_b {
            map_b
                .entry(m.metric_name.as_str())
                .or_default()
                .push(m.metric_value);
        }

        let mut names: Vec<&str> = map_a.keys().copied().collect();
        names.extend(map_b.keys().copied());
        names.sort_unstable();
        names.dedup();

        names
            .into_iter()
            .map(|name| {
                let values_a = MetricValues::new(map_a.get(name).cloned().unwrap_or_default());
                let values_b = MetricValues::new(map_b.get(name).cloned().unwrap_or_default());
                let deltas = MetricDeltas::from_left_right(&values_a, &values_b);
                let direction = MetricDirection::from_deltas(&deltas);
                let same = values_a.0 == values_b.0;

                Self {
                    name: name.to_owned(),
                    run_a: values_a,
                    run_b: values_b,
                    same,
                    deltas,
                    direction,
                }
            })
            .collect()
    }
}
