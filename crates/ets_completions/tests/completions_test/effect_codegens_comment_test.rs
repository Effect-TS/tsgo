use crate::*;

// go test: internal/completions/effect_codegens_comment_test.go:11 TestEffectCodegensComment_Triggers 3cf933a223a1734f
#[test]
fn test_effect_codegens_comment_triggers() {
    go_test(|t| {
        t.parallel();
        run_comment_directive_trigger_cases(t, &["@effect-codegens"]);
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:16 TestEffectCodegensComment_SnippetContainsSortedCodegenNames 0dee41e979d8a35a
#[test]
fn test_effect_codegens_comment_snippet_contains_sorted_codegen_names() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = labeled_items(t, source, source.len(), &["@effect-codegens"]);
        if items.len() != 1 {
            t.fatal(format!("expected 1 item, got {}", items.len()));
        }

        let mut names: Vec<&str> = ets_codegens::ALL.iter().map(|c| c.name).collect();
        names.sort_unstable();
        let sorted_names = names.join(",");

        let insert_text = new_text(&items[0]);
        if !insert_text.contains(&sorted_names) {
            t.error(format!(
                "insert text does not contain sorted codegen names.\ngot:  {insert_text:?}\nwant substring: {sorted_names:?}"
            ));
        }
    });
}
