use crate::*;

// go test: internal/completions/effect_codegens_comment_test.go:11 TestEffectCodegensComment_DoubleSlash
#[test]
fn test_effect_codegens_comment_double_slash() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for '// @', got {}", items.len()));
        }
        if items[0].label != "@effect-codegens" {
            t.error(format!("item[0].Label = {:?}, want {:?}", items[0].label, "@effect-codegens"));
        }
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:23 TestEffectCodegensComment_SlashStar
#[test]
fn test_effect_codegens_comment_slash_star() {
    go_test(|t| {
        t.parallel();
        let source = "/* @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for '/* @', got {}", items.len()));
        }
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:32 TestEffectCodegensComment_JSDocComment
#[test]
fn test_effect_codegens_comment_jsdoc_comment() {
    go_test(|t| {
        t.parallel();
        let source = "/** @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for '/** @', got {}", items.len()));
        }
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:41 TestEffectCodegensComment_ExtraWhitespace
#[test]
fn test_effect_codegens_comment_extra_whitespace() {
    go_test(|t| {
        t.parallel();
        let source = "//  @  ";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!(
                "expected 1 item for '//  @  ' (extra whitespace), got {}",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:50 TestEffectCodegensComment_NoAtSymbol
#[test]
fn test_effect_codegens_comment_no_at_symbol() {
    go_test(|t| {
        t.parallel();
        let source = "// some comment without at";
        let items = effect_codegens_comment_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for comment without @, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:59 TestEffectCodegensComment_AtOutsideComment
#[test]
fn test_effect_codegens_comment_at_outside_comment() {
    go_test(|t| {
        t.parallel();
        let source = "const x = @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for @ outside comment, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_codegens_comment_test.go:68 TestEffectCodegensComment_SnippetContainsSortedCodegenNames
#[test]
fn test_effect_codegens_comment_snippet_contains_sorted_codegen_names() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item, got {}", items.len()));
        }

        // Build expected sorted names
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

// go test: internal/completions/effect_codegens_comment_test.go:90 TestEffectCodegensComment_ReplacementSpanStartsAtAt
#[test]
fn test_effect_codegens_comment_replacement_span_starts_at_at() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item, got {}", items.len()));
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

// go test: internal/completions/effect_codegens_comment_test.go:105 TestEffectCodegensComment_MultilineWithCommentOnSecondLine
#[test]
fn test_effect_codegens_comment_multiline_with_comment_on_second_line() {
    go_test(|t| {
        t.parallel();
        let source = "const x = 1\n// @";
        let items = effect_codegens_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for multiline source, got {}", items.len()));
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
