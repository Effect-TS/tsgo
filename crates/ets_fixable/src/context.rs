use std::sync::Arc;

use ets_etscore::ResolvedEffectPluginOptions;
use ets_rewriter::Tracker;
use ets_shim::ast::SourceFile;
use ets_shim::checker::Checker;
use ets_shim::compiler::Program;
use ets_shim::core::context::Context as GoContext;
use ets_shim::core::{P, TextPos, TextRange};
use ets_shim::ls::lsconv::{self, Converters};
use ets_shim::ls::lsutil::{self, FormatCodeSettings};
use ets_shim::ls::{self, change};
use ets_shim::lsproto;
use ets_typeparser::TypeParser;

// go: internal/fixable/context.go:24 Context da925ab1eb218917
// Context bundles the code-fix request data and provides helpers for fixable implementations.
// It replaces the (context.Context, *rewriter.Tracker, *ls.CodeFixContext) parameter triple,
// giving each fixable self-contained access to the checker, tracker lifecycle, and edit finalization.
//
// Rust: Go holds both `Checker` and `TypeParser`; `ets_typeparser::TypeParser` borrows the checker mutably for its
// lifetime, so the context holds the parser and reaches the checker through it (`checker()`), as
// `ets_rule::Context` does. `Options` (Go: a pointer, nil when absent) is borrowed.
pub struct Context<'a, 'c> {
    pub source_file: P<SourceFile>,
    pub span: TextRange,
    pub error_code: i32,
    pub options: Option<&'a ResolvedEffectPluginOptions>,
    pub program: &'static Program,
    pub type_parser: TypeParser<'c>,

    pub context: GoContext,

    pub(crate) format_options: FormatCodeSettings,
    pub(crate) converters: Arc<Converters>,
}

impl Context<'_, '_> {
    /// Rust-only: Go's `Checker` field (the checker `type_parser` runs on; see the struct).
    pub fn checker(&mut self) -> &mut Checker {
        self.type_parser.checker()
    }
}

// go: internal/fixable/context.go:40 NewContext 014495efdfc2bd67
// NewContext creates a fixable Context from the standard code-fix request parameters.
//
// Rust: Go panics on a nil program or checker; neither can be nil here. The checker is `tp`'s (see `Context`).
pub fn new_context<'a, 'c>(
    ctx: &GoContext,
    fix_ctx: &ls::CodeFixContext<'_>,
    options: Option<&'a ResolvedEffectPluginOptions>,
    tp: TypeParser<'c>,
) -> Context<'a, 'c> {
    Context {
        source_file: fix_ctx.source_file,
        span: fix_ctx.span,
        error_code: fix_ctx.error_code,
        options,
        program: fix_ctx.program,
        type_parser: tp,
        context: ctx.clone(),
        format_options: fix_ctx.ls.format_options(),
        // Go's shim accessor `ls.LanguageService_converters` (tsrs patch 100).
        converters: Arc::clone(&fix_ctx.ls.converters),
    }
}

// go: internal/fixable/context.go:62 NewStandaloneContext 3a25a94c7f60328b
// NewStandaloneContext creates a fixable Context without a language service.
//
// Rust: Go panics on a nil program or checker; neither can be nil here. The checker is `tp`'s (see `Context`).
pub fn new_standalone_context<'a, 'c>(
    ctx: &GoContext,
    source_file: P<SourceFile>,
    span: TextRange,
    error_code: i32,
    options: Option<&'a ResolvedEffectPluginOptions>,
    program: &'static Program,
    tp: TypeParser<'c>,
) -> Context<'a, 'c> {
    let converters = new_standalone_converters(source_file);
    Context {
        source_file,
        span,
        error_code,
        options,
        program,
        type_parser: tp,
        context: ctx.clone(),
        format_options: lsutil::get_default_format_code_settings(),
        converters,
    }
}

// go: internal/fixable/context.go:93 newStandaloneConverters 34f75ad96156b548
pub(crate) fn new_standalone_converters(source_file: P<SourceFile>) -> Arc<Converters> {
    let file_name = source_file.file_name().to_string();
    let text = source_file.text();
    lsconv::new_converters(lsproto::PositionEncodingKind::UTF16, move |name: &str| {
        if name == file_name {
            return Some(lsconv::compute_lsp_line_starts(text));
        }
        None
    })
}

impl Context<'_, '_> {
    // go: internal/fixable/context.go:103 Context.BytePosToLSPPosition 35cc75e1220e2f18
    // BytePosToLSPPosition converts a byte offset using the context's LSP encoding.
    pub fn byte_pos_to_lsp_position(&self, pos: u32) -> lsproto::Position {
        let (position, _) = self.converters.to_lsp_position(&self.source_file, TextPos::from(pos));
        position
    }

    // go: internal/fixable/context.go:109 Context.LSPRangeToTextRange a946ab9f50b7b6ab
    // LSPRangeToTextRange converts a tracker edit range back to source byte offsets.
    pub fn lsp_range_to_text_range(&self, r: lsproto::Range) -> TextRange {
        self.converters.from_lsp_range_to_original(&self.source_file, r)
    }
}

// go: internal/fixable/context.go:114 FixAction 5ac38e672a866d8f
// FixAction describes a single code action that a fixable wants to produce.
//
// Rust: `run` is called once (Go's closure), so it may move what it captures.
pub struct FixAction<'f> {
    pub description: String,
    pub run: Box<dyn FnOnce(&mut Tracker) + 'f>,
}

impl Context<'_, '_> {
    // go: internal/fixable/context.go:122 Context.NewFixAction 030b4e9040996ea0
    // NewFixAction creates a tracker, runs the action's edit closure, and returns
    // a *ls.CodeAction wrapping the resulting edits for the current SourceFile.
    // Returns nil if the closure produced no edits.
    pub fn new_fix_action(&self, action: FixAction<'_>) -> Option<ls::CodeAction> {
        let raw_tracker = change::new_tracker(
            &self.context,
            &self.program.options(),
            self.format_options.clone(),
            Arc::clone(&self.converters),
        );
        let mut tracker = ets_rewriter::new_tracker(raw_tracker);
        (action.run)(&mut tracker);
        let edits =
            tracker.get_changes().shift_remove(self.source_file.file_name()).unwrap_or_default();
        if edits.is_empty() {
            return None;
        }
        Some(ls::CodeAction {
            description: action.description,
            changes: edits,
            ..Default::default()
        })
    }
}
