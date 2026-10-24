use crate::*;

// go test: internal/completions/effect_jsdoc_comment_test.go:7 TestEffectJsdocComment_DoubleSlash
#[test]
fn test_effect_jsdoc_comment_double_slash() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for '// @', got {}", items.len()));
        }
        if items[0].label != "@effect-identifier" {
            t.error(format!(
                "item[0].Label = {:?}, want {:?}",
                items[0].label, "@effect-identifier"
            ));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:19 TestEffectJsdocComment_SlashStar
#[test]
fn test_effect_jsdoc_comment_slash_star() {
    go_test(|t| {
        t.parallel();
        let source = "/* @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for '/* @', got {}", items.len()));
        }
        if items[0].label != "@effect-identifier" {
            t.error(format!(
                "item[0].Label = {:?}, want {:?}",
                items[0].label, "@effect-identifier"
            ));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:31 TestEffectJsdocComment_JSDocComment
#[test]
fn test_effect_jsdoc_comment_jsdoc_comment() {
    go_test(|t| {
        t.parallel();
        let source = "/** @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item for '/** @', got {}", items.len()));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:40 TestEffectJsdocComment_ExtraWhitespace
#[test]
fn test_effect_jsdoc_comment_extra_whitespace() {
    go_test(|t| {
        t.parallel();
        let source = "//  @  ";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!(
                "expected 1 item for '//  @  ' (extra whitespace), got {}",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:49 TestEffectJsdocComment_NoAtSymbol
#[test]
fn test_effect_jsdoc_comment_no_at_symbol() {
    go_test(|t| {
        t.parallel();
        let source = "// some comment";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for comment without @, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:58 TestEffectJsdocComment_AtOutsideComment
#[test]
fn test_effect_jsdoc_comment_at_outside_comment() {
    go_test(|t| {
        t.parallel();
        let source = "const x = @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for @ outside comment, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:67 TestEffectJsdocComment_InsertText
#[test]
fn test_effect_jsdoc_comment_insert_text() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
        if items.len() != 1 {
            t.fatal(format!("expected 1 item, got {}", items.len()));
        }

        let insert_text = new_text(&items[0]);
        if insert_text != "@effect-identifier" {
            t.error(format!("insert text = {insert_text:?}, want {:?}", "@effect-identifier"));
        }
    });
}

// go test: internal/completions/effect_jsdoc_comment_test.go:81 TestEffectJsdocComment_ReplacementSpanStartsAtAt
#[test]
fn test_effect_jsdoc_comment_replacement_span_starts_at_at() {
    go_test(|t| {
        t.parallel();
        let source = "// @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
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

// go test: internal/completions/effect_jsdoc_comment_test.go:96 TestEffectJsdocComment_MultilineWithCommentOnSecondLine
#[test]
fn test_effect_jsdoc_comment_multiline_with_comment_on_second_line() {
    go_test(|t| {
        t.parallel();
        let source = "const x = 1\n// @";
        let items = effect_jsdoc_comment_items(t, source, source.len());
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
