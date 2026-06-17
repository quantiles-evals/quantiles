use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::{Result, bail};
use serde_json::Value;

use crate::builtins::common::extract_text;

/// A normalized `PubMedQA` row after messy upstream data is coerced.
#[derive(Debug)]
pub(crate) struct PubmedQARow {
    pub(crate) sample_id: String,
    pub(crate) question: String,
    pub(crate) context: String,
    pub(crate) gold_answer: String,
}

/// Transform a raw HF row into a canonical `PubmedQARow`.
pub(crate) fn transform_pubmedqa_row(raw: &Value) -> Result<PubmedQARow> {
    let question = extract_text(raw, "question")
        .or_else(|| extract_text(raw, "query"))
        .or_else(|| extract_text(raw, "prompt"))
        .or_else(|| extract_text(raw, "input"))
        .unwrap_or_default();

    let context = extract_context(raw);

    let gold_answer = normalize_label(raw.get("final_decision"))
        .or_else(|| normalize_label(raw.get("finalDecision")))
        .or_else(|| normalize_label(raw.get("answer")))
        .or_else(|| normalize_label(raw.get("label")))
        .or_else(|| normalize_label(raw.get("target")));

    if question.is_empty() || gold_answer.is_none() {
        bail!("missing question or gold_answer");
    }

    let sample_id = extract_text(raw, "id")
        .or_else(|| extract_text(raw, "qid"))
        .or_else(|| extract_text(raw, "question_id"))
        .unwrap_or_else(|| {
            let mut hasher = DefaultHasher::new();
            format!("{}|{}|{}", question, context, gold_answer.as_ref().unwrap()).hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        });

    Ok(PubmedQARow {
        sample_id,
        question,
        context,
        gold_answer: gold_answer.unwrap(),
    })
}

/// Coerce arbitrary JSON into a plain string (handles nested lists/objects).
fn coerce_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.trim().to_string(),
        Value::Array(arr) => arr
            .iter()
            .map(coerce_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => map
            .values()
            .map(coerce_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Extract the context field, trying multiple upstream column names and nested structures.
fn extract_context(row: &Value) -> String {
    let direct_keys = ["context", "abstract", "passage", "long_answer", "evidence"];
    for key in &direct_keys {
        if let Some(text) = extract_text(row, key)
            && !text.is_empty()
        {
            return text;
        }
    }

    if let Some(context) = row.get("context") {
        let text = coerce_ordered_object(context, &["contexts", "labels", "meshes"]);
        if !text.is_empty() {
            return text;
        }
    }

    if let Some(contexts) = row.get("contexts") {
        let text = coerce_ordered_object(
            contexts,
            &["label", "contexts", "context", "abstract", "title"],
        );
        if !text.is_empty() {
            return text;
        }
    }

    String::new()
}

fn coerce_ordered_object(value: &Value, ordered_keys: &[&str]) -> String {
    match value {
        Value::Array(arr) => arr
            .iter()
            .map(coerce_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => {
            let mut parts = Vec::new();
            for key in ordered_keys {
                if let Some(v) = map.get(*key) {
                    let text = coerce_text(v);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
            }
            for (key, value) in map {
                if !ordered_keys.contains(&key.as_str()) {
                    let text = coerce_text(value);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

/// Normalize a raw label value to yes/no/maybe.
fn normalize_label(value: Option<&Value>) -> Option<String> {
    let s = value?.as_str()?;
    let normalized = s.trim().to_lowercase();
    if ["yes", "no", "maybe"].contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use serde_json::json;

    #[fixture]
    fn standard_row() -> Value {
        json!({
            "question": "Is it effective?",
            "context": "Study shows 90% efficacy.",
            "final_decision": "yes",
            "id": "q1"
        })
    }

    #[fixture]
    fn alias_row() -> Value {
        json!({
            "query": "Does it work?",
            "abstract": "Results were promising.",
            "answer": "maybe",
            "qid": "q2"
        })
    }

    #[fixture]
    fn nested_object_context_row() -> Value {
        json!({
            "prompt": "Is it safe?",
            "contexts": {
                "label": "Primary",
                "context": "No adverse events reported.",
                "title": "Safety Study"
            },
            "label": "yes",
            "question_id": "q3"
        })
    }

    #[fixture]
    fn pubmedqa_context_row() -> Value {
        json!({
            "question": "Do statins reduce atrial fibrillation?",
            "context": {
                "contexts": [
                    "Preoperative statin therapy was evaluated.",
                    "Postoperative atrial fibrillation was less frequent."
                ],
                "labels": ["BACKGROUND", "RESULTS"],
                "meshes": ["Atrial Fibrillation", "Hydroxymethylglutaryl-CoA Reductase Inhibitors"]
            },
            "final_decision": "yes",
            "pubid": 12345
        })
    }

    #[fixture]
    fn list_context_row() -> Value {
        json!({
            "input": "Should we use it?",
            "contexts": [
                "Phase I completed.",
                "Phase II in progress."
            ],
            "target": "no",
            "id": "q4"
        })
    }

    #[rstest]
    fn test_transform_standard(standard_row: Value) {
        let row = transform_pubmedqa_row(&standard_row).unwrap();
        assert_eq!(row.sample_id, "q1");
        assert_eq!(row.question, "Is it effective?");
        assert_eq!(row.context, "Study shows 90% efficacy.");
        assert_eq!(row.gold_answer, "yes");
    }

    #[rstest]
    fn test_transform_alias_fields(alias_row: Value) {
        let row = transform_pubmedqa_row(&alias_row).unwrap();
        assert_eq!(row.sample_id, "q2");
        assert_eq!(row.question, "Does it work?");
        assert_eq!(row.context, "Results were promising.");
        assert_eq!(row.gold_answer, "maybe");
    }

    #[rstest]
    fn test_transform_nested_object_context(nested_object_context_row: Value) {
        let row = transform_pubmedqa_row(&nested_object_context_row).unwrap();
        assert_eq!(row.sample_id, "q3");
        assert_eq!(row.question, "Is it safe?");
        assert_eq!(
            row.context,
            "Primary\nNo adverse events reported.\nSafety Study"
        );
        assert_eq!(row.gold_answer, "yes");
    }

    #[rstest]
    fn test_transform_pubmedqa_nested_context(pubmedqa_context_row: Value) {
        let row = transform_pubmedqa_row(&pubmedqa_context_row).unwrap();
        assert_eq!(row.question, "Do statins reduce atrial fibrillation?");
        assert_eq!(
            row.context,
            "Preoperative statin therapy was evaluated.\n\
            Postoperative atrial fibrillation was less frequent.\n\
            BACKGROUND\n\
            RESULTS\n\
            Atrial Fibrillation\n\
            Hydroxymethylglutaryl-CoA Reductase Inhibitors"
        );
        assert_eq!(row.gold_answer, "yes");
    }

    #[rstest]
    fn test_transform_list_context(list_context_row: Value) {
        let row = transform_pubmedqa_row(&list_context_row).unwrap();
        assert_eq!(row.sample_id, "q4");
        assert_eq!(row.question, "Should we use it?");
        assert_eq!(row.context, "Phase I completed.\nPhase II in progress.");
        assert_eq!(row.gold_answer, "no");
    }

    #[rstest]
    fn test_transform_missing_question() {
        let raw = json!({"context": "Some context", "final_decision": "yes"});
        assert!(transform_pubmedqa_row(&raw).is_err());
    }

    #[rstest]
    fn test_transform_missing_gold_answer() {
        let raw = json!({"question": "Some question?", "context": "Some context"});
        assert!(transform_pubmedqa_row(&raw).is_err());
    }

    #[rstest]
    fn test_transform_generates_sample_id_when_missing() {
        let raw = json!({
            "question": "Q",
            "context": "C",
            "final_decision": "yes"
        });
        let row = transform_pubmedqa_row(&raw).unwrap();
        assert!(!row.sample_id.is_empty());
        assert_eq!(row.sample_id.len(), 16);
    }

    #[rstest]
    #[case(json!({"context": "Direct"}), "Direct")]
    #[case(json!({"abstract": "Abstract text"}), "Abstract text")]
    #[case(json!({"passage": "Passage text"}), "Passage text")]
    #[case(json!({"long_answer": "Long"}), "Long")]
    #[case(json!({"evidence": "Evidence"}), "Evidence")]
    fn test_extract_context_direct_fields(#[case] input: Value, #[case] expected: &str) {
        assert_eq!(extract_context(&input), expected);
    }

    #[rstest]
    fn test_extract_context_list() {
        let input = json!({"contexts": ["Part 1", "Part 2"]});
        assert_eq!(extract_context(&input), "Part 1\nPart 2");
    }

    #[rstest]
    fn test_extract_context_nested_object() {
        let input = json!({
            "contexts": {
                "label": "L",
                "context": "C",
                "abstract": "A",
                "title": "T",
                "extra": "E"
            }
        });
        assert_eq!(extract_context(&input), "L\nC\nA\nT\nE");
    }

    #[rstest]
    fn test_extract_context_nested_singular_context() {
        let input = json!({
            "context": {
                "contexts": ["C1", "C2"],
                "labels": ["L1", "L2"],
                "meshes": ["M1"],
                "extra": "E"
            }
        });
        assert_eq!(extract_context(&input), "C1\nC2\nL1\nL2\nM1\nE");
    }

    #[rstest]
    fn test_extract_context_empty() {
        assert_eq!(extract_context(&json!({})), "");
    }

    #[rstest]
    fn test_extract_context_prefers_direct_over_nested() {
        let input = json!({"context": "Direct", "contexts": ["Nested"]});
        assert_eq!(extract_context(&input), "Direct");
    }

    #[rstest]
    #[case(json!("hello"), "hello")]
    #[case(json!("  hello  "), "hello")]
    #[case(json!(["a", "b"]), "a\nb")]
    #[case(json!({"x": "a", "y": "b"}), "a\nb")]
    #[case(json!(null), "")]
    #[case(json!(42), "")]
    fn test_coerce_text(#[case] value: Value, #[case] expected: &str) {
        assert_eq!(coerce_text(&value), expected);
    }
}
