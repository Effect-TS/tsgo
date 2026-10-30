use crate::*;

// go test: internal/completions/effect_diagnostics_comment_test.go:11 TestEffectDiagnosticsComment_Triggers 749aed41aef86e0e
#[test]
fn test_effect_diagnostics_comment_triggers() {
    go_test(|t| {
        t.parallel();
        run_comment_directive_trigger_cases(
            t,
            &["@effect-diagnostics", "@effect-diagnostics-next-line"],
        );
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:16 TestEffectDiagnosticsComment_SnippetChoices edff4991b8f1d2d4
#[test]
fn test_effect_diagnostics_comment_snippet_choices() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = labeled_items(
            t,
            source,
            source.len(),
            &["@effect-diagnostics", "@effect-diagnostics-next-line"],
        );
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        let mut names: Vec<&str> = ets_rules::ALL.iter().map(|r| r.name).collect();
        names.sort_unstable();

        let insert_text = new_text(&items[0]);
        for expected in [names.join(","), "off,warning,error,message,suggestion".to_string()] {
            if !insert_text.contains(&expected) {
                t.error(format!(
                    "insert text does not contain choices.\ngot:  {insert_text:?}\nwant substring: {expected:?}"
                ));
            }
        }
    });
}
