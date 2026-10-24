use crate::*;

// go test: internal/completions/effect_diagnostics_comment_test.go:11 TestEffectDiagnosticsComment_DoubleSlash
#[test]
fn test_effect_diagnostics_comment_double_slash() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items for '// @', got {}", items.len()));
        }
        if items[0].label != "@effect-diagnostics" {
            t.error(format!(
                "item[0].Label = {:?}, want {:?}",
                items[0].label, "@effect-diagnostics"
            ));
        }
        if items[1].label != "@effect-diagnostics-next-line" {
            t.error(format!(
                "item[1].Label = {:?}, want {:?}",
                items[1].label, "@effect-diagnostics-next-line"
            ));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:26 TestEffectDiagnosticsComment_SlashStar
#[test]
fn test_effect_diagnostics_comment_slash_star() {
    go_test(|t| {
        t.parallel();
        let source = "/* @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items for '/* @', got {}", items.len()));
        }
        if items[0].label != "@effect-diagnostics" {
            t.error(format!(
                "item[0].Label = {:?}, want {:?}",
                items[0].label, "@effect-diagnostics"
            ));
        }
        if items[1].label != "@effect-diagnostics-next-line" {
            t.error(format!(
                "item[1].Label = {:?}, want {:?}",
                items[1].label, "@effect-diagnostics-next-line"
            ));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:41 TestEffectDiagnosticsComment_JSDocComment
#[test]
fn test_effect_diagnostics_comment_jsdoc_comment() {
    go_test(|t| {
        t.parallel();
        let source = "/** @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items for '/** @', got {}", items.len()));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:50 TestEffectDiagnosticsComment_ExtraWhitespace
#[test]
fn test_effect_diagnostics_comment_extra_whitespace() {
    go_test(|t| {
        t.parallel();
        let source = "//  @  ";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!(
                "expected 2 items for '//  @  ' (extra whitespace), got {}",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:59 TestEffectDiagnosticsComment_NoAtSymbol
#[test]
fn test_effect_diagnostics_comment_no_at_symbol() {
    go_test(|t| {
        t.parallel();
        let source = "// some comment without at";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for comment without @, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:68 TestEffectDiagnosticsComment_AtOutsideComment
#[test]
fn test_effect_diagnostics_comment_at_outside_comment() {
    go_test(|t| {
        t.parallel();
        let source = "const x = @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for @ outside comment, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:77 TestEffectDiagnosticsComment_SnippetContainsSortedRuleNames
#[test]
fn test_effect_diagnostics_comment_snippet_contains_sorted_rule_names() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        // Build expected sorted names
        let mut names: Vec<&str> = ets_rules::ALL.iter().map(|r| r.name).collect();
        names.sort_unstable();
        let sorted_names = names.join(",");

        let insert_text = new_text(&items[0]);
        if !insert_text.contains(&sorted_names) {
            t.error(format!(
                "insert text does not contain sorted rule names.\ngot:  {insert_text:?}\nwant substring: {sorted_names:?}"
            ));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:99 TestEffectDiagnosticsComment_SnippetContainsSeverityChoices
#[test]
fn test_effect_diagnostics_comment_snippet_contains_severity_choices() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        let expected = "off,warning,error,message,suggestion";
        let insert_text = new_text(&items[0]);
        if !insert_text.contains(expected) {
            t.error(format!(
                "insert text does not contain severity choices.\ngot:  {insert_text:?}\nwant substring: {expected:?}"
            ));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:114 TestEffectDiagnosticsComment_ReplacementSpanStartsAtAt
#[test]
fn test_effect_diagnostics_comment_replacement_span_starts_at_at() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        // The @ is at byte offset 3 in "// @", which is line 0, character 3
        let rang = edit_range(&items[0]);
        if rang.start.character != 3 {
            t.error(format!(
                "replacement range start character = {}, want 3",
                rang.start.character
            ));
        }
    });
}

// go test: internal/completions/effect_diagnostics_comment_test.go:129 TestEffectDiagnosticsComment_MultilineWithCommentOnSecondLine
#[test]
fn test_effect_diagnostics_comment_multiline_with_comment_on_second_line() {
    go_test(|t| {
        t.parallel();
        let source = "const x = 1\n// @";
        let items = effect_diagnostics_comment_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items for multiline source, got {}", items.len()));
        }

        // @ is at character 3 on line 1
        let rang = edit_range(&items[0]);
        if rang.start.line != 1 {
            t.error(format!("replacement range start line = {}, want 1", rang.start.line));
        }
        if rang.start.character != 3 {
            t.error(format!(
                "replacement range start character = {}, want 3",
                rang.start.character
            ));
        }
    });
}
