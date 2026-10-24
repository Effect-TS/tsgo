// Package fixables contains all code fix implementations.

use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::SourceFile;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;

// go: internal/fixables/disable.go:17 EffectDisable
// EffectDisable provides "disable for line/file" actions for all Effect diagnostics.
// This fixable handles all Effect diagnostic codes and provides two actions:
// 1. "Disable {ruleName} for this line" - inserts @effect-diagnostics-next-line directive
// 2. "Disable {ruleName} for entire file" - inserts @effect-diagnostics skip-file directive
pub static EFFECT_DISABLE: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectDisable",
    description: "Disable Effect diagnostics via directive comments",
    error_codes: ets_rule::all_codes(&ets_rules::ALL),
    fix_ids: vec!["effectDisableNextLine", "effectDisableFile"],
    run: run_effect_disable,
});

// go: internal/fixables/disable.go:26 runEffectDisable
// runEffectDisable generates disable actions for Effect diagnostics.
fn run_effect_disable(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let rule_name = ets_rule::code_to_rule_name(&ets_rules::ALL, ctx.error_code);
    if rule_name.is_empty() {
        return Vec::new();
    }

    let mut actions = Vec::new();

    // "Disable for this line" action
    if let Some(action) = create_disable_next_line_action(ctx, rule_name) {
        actions.push(action);
    }

    // "Disable for entire file" action
    if let Some(action) = create_disable_file_action(ctx, rule_name) {
        actions.push(action);
    }

    actions
}

// go: internal/fixables/disable.go:49 createDisableNextLineAction
// createDisableNextLineAction creates a code action to disable the diagnostic for the next line.
// It inserts: // @effect-diagnostics-next-line {ruleName}:off
fn create_disable_next_line_action(ctx: &Context<'_, '_>, rule_name: &str) -> Option<CodeAction> {
    let source_file = ctx.source_file;
    let span = ctx.span;

    // Find the start of the line containing the diagnostic
    let line_start_pos = get_line_start_position_for_position(
        span.pos().expect_source("a diagnostic span"),
        source_file,
    );

    // Get the indentation at this position
    let indent = get_indentation_at_position(source_file, line_start_pos);

    // Create the comment text with proper indentation
    let comment = format!("{indent}// @effect-diagnostics-next-line {rule_name}:off\n");

    // Insert at the start of the line
    let mut insert_pos = ctx.byte_pos_to_lsp_position(line_start_pos);
    // Character must be 0 since we're inserting at line start with full indentation in the text
    insert_pos.character = 0;

    ctx.new_fix_action(FixAction {
        description: format!("Disable {rule_name} for this line"),
        run: Box::new(move |tracker| {
            tracker.insert_text(source_file, insert_pos, &comment);
        }),
    })
}

// go: internal/fixables/disable.go:77 createDisableFileAction
// createDisableFileAction creates a code action to disable the diagnostic for the entire file.
// It inserts: /** @effect-diagnostics {ruleName}:off */ at the top of the file.
fn create_disable_file_action(ctx: &Context<'_, '_>, rule_name: &str) -> Option<CodeAction> {
    // Create the comment text (uses :off for consistency with next-line directives)
    let comment = format!("/** @effect-diagnostics {rule_name}:off */\n");

    // Insert at the very beginning of the file (position 0)
    let insert_pos = ctx.byte_pos_to_lsp_position(0);

    let source_file = ctx.source_file;
    ctx.new_fix_action(FixAction {
        description: format!("Disable {rule_name} for entire file"),
        run: Box::new(move |tracker| {
            tracker.insert_text(source_file, insert_pos, &comment);
        }),
    })
}

// go: internal/fixables/disable.go:93 getLineStartPositionForPosition
// getLineStartPositionForPosition finds the start of the line containing the given position.
fn get_line_start_position_for_position(pos: u32, source_file: P<SourceFile>) -> u32 {
    let text = source_file.text().as_bytes();
    if pos == 0 || pos as usize > text.len() {
        return 0;
    }
    // Walk backwards to find the newline
    for i in (0..pos as usize).rev() {
        if text[i] == b'\n' {
            return u32::try_from(i + 1).unwrap();
        }
    }
    0
}

// go: internal/fixables/disable.go:108 getIndentationAtPosition
// getIndentationAtPosition returns the whitespace characters from line start to the first non-whitespace.
fn get_indentation_at_position(source_file: P<SourceFile>, line_start_pos: u32) -> &'static str {
    let text = source_file.text();
    let line_start_pos = line_start_pos as usize;
    if line_start_pos >= text.len() {
        return "";
    }

    // Find the end of indentation (first non-whitespace character or end of line)
    let mut end_pos = line_start_pos;
    while end_pos < text.len() {
        let ch = text.as_bytes()[end_pos];
        if ch == b' ' || ch == b'\t' {
            end_pos += 1;
        } else {
            break;
        }
    }

    &text[line_start_pos..end_pos]
}
