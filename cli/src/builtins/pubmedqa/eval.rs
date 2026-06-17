/// Build the single-string prompt sent to the sampler.
pub(crate) fn build_prompt(question: &str, context: &str) -> String {
    format!(
        "You are answering a biomedical research question.\n\
        Reply with exactly one label: yes, no, or maybe.\n\n\
         Question:\n{question}\n\n\
         Context:\n{context}\n\n\
         Answer with exactly one token: yes, no, or maybe."
    )
}

/// Extract a `PubMedQA` label (`yes`, `no`, or `maybe`) from a model response.
///
/// This function is intentionally tolerant of common LLM formatting variations,
/// so it can accept responses including the following:
///
/// - "yes"
/// - "Yes."
/// - "The answer is no."
/// - "maybe, based on the abstract"
///
/// The parser only examines the first few tokens of the response to avoid
/// accidentally matching unrelated words later in a long generation.
pub(crate) fn extract_label_from_response(content: &str) -> Option<String> {
    let lowered = content.trim().to_lowercase();
    match lowered.as_str() {
        "yes" | "no" | "maybe" => {
            return Some(lowered);
        }
        _ => {}
    }

    let cleaned = lowered.trim_start_matches(|c: char| !c.is_alphabetic());

    for token in cleaned.split_whitespace().take(5) {
        let token = token.trim_matches(|c: char| !c.is_alphabetic());
        match token {
            "yes" | "no" | "maybe" => {
                return Some(token.to_string());
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("yes", Some("yes"))]
    #[case("no", Some("no"))]
    #[case("maybe", Some("maybe"))]
    #[case("Yes", Some("yes"))]
    #[case("NO", Some("no"))]
    #[case("Maybe", Some("maybe"))]
    #[case("yes.", Some("yes"))]
    #[case("no,", Some("no"))]
    #[case("(maybe)", Some("maybe"))]
    #[case("The answer is yes.", Some("yes"))]
    #[case("I think no.", Some("no"))]
    #[case("It is maybe, based on evidence.", Some("maybe"))]
    #[case("  \n\nyes", Some("yes"))]
    #[case("\t no", Some("no"))]
    #[case("\"yes\"", Some("yes"))]
    #[case("**maybe**", Some("maybe"))]
    #[case("aB3fG9kL2m", None)]
    #[case("hello world", None)]
    #[case("", None)]
    #[case("one two three four five yes", None)]
    fn test_extract_label_from_response(#[case] input: &str, #[case] expected: Option<&str>) {
        assert_eq!(
            extract_label_from_response(input),
            expected.map(String::from)
        );
    }

    #[rstest]
    #[case("Q1", "C1")]
    #[case("A longer question?", "A longer context.")]
    #[case("", "")]
    fn test_build_prompt_contains_parts(#[case] question: &str, #[case] context: &str) {
        let prompt = build_prompt(question, context);
        assert!(prompt.contains("yes, no, or maybe"));
        assert!(prompt.contains(question));
        assert!(prompt.contains(context));
    }
}
