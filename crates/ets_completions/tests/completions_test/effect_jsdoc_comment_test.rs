use crate::*;

// go test: internal/completions/effect_jsdoc_comment_test.go:7 TestEffectJsdocComment_Triggers 525615a1deddb096
#[test]
fn test_effect_jsdoc_comment_triggers() {
    go_test(|t| {
        t.parallel();
        run_comment_directive_trigger_cases(t, &["@effect-identifier"]);
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:12 TestEffectJsdocComment_InsertText 52d86b0ec6bac548
#[test]
fn test_effect_jsdoc_comment_insert_text() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = labeled_items(t, source, source.len(), &["@effect-identifier"]);
        if items.len() != 1 {
            t.fatal(format!("expected 1 item, got {}", items.len()));
        }

        let insert_text = new_text(&items[0]);
        if insert_text != "@effect-identifier" {
            t.error(format!("insert text = {insert_text:?}, want {:?}", "@effect-identifier"));
        }
    });
}
