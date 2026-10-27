use std::sync::Arc;

use ets_rewriter::Tracker;
use ets_shim::ast::SourceFile;
use ets_shim::checker::Checker;
use ets_shim::compiler::Program;
use ets_shim::core::context::Context as GoContext;
use ets_shim::core::{P, TextRange};
use ets_shim::ls::{self, LanguageService, change};
use ets_shim::lsproto;
use ets_typeparser::TypeParser;

// go: internal/refactor/context.go:20 Context 7cbdd1d788ed89e3
// Context bundles the refactor request data and provides helpers for refactor implementations.
// Unlike fixable.Context, there is no ErrorCode — refactors apply to any selection.
//
// Rust: Go holds both `Checker` and `TypeParser`; `ets_typeparser::TypeParser` borrows the checker mutably for its
// lifetime, so the context holds the parser and reaches the checker through it (`checker()`), as
// `ets_fixable::Context` does.
pub struct Context<'a, 'c> {
    pub source_file: P<SourceFile>,
    pub span: TextRange,
    pub program: &'static Program,
    pub type_parser: TypeParser<'c>,

    pub context: GoContext,
    pub(crate) ls: &'a LanguageService,
}

impl Context<'_, '_> {
    /// Rust-only: Go's `Checker` field (the checker `type_parser` runs on; see the struct).
    pub fn checker(&mut self) -> &mut Checker {
        self.type_parser.checker()
    }
}

// go: internal/refactor/context.go:32 NewContext 0ac9df00ca376f29
// NewContext creates a refactor Context from the refactor provider callback parameters.
//
// Rust: Go panics on a nil program or checker; neither can be nil here. The checker is `tp`'s (see `Context`).
pub fn new_context<'a, 'c>(
    ctx: &GoContext,
    source_file: P<SourceFile>,
    span: TextRange,
    program: &'static Program,
    lang_service: &'a LanguageService,
    tp: TypeParser<'c>,
) -> Context<'a, 'c> {
    Context { source_file, span, program, type_parser: tp, context: ctx.clone(), ls: lang_service }
}

impl Context<'_, '_> {
    // go: internal/refactor/context.go:52 Context.BytePosToLSPPosition 005a0ec586ac9e72
    // BytePosToLSPPosition converts a single byte offset in the context's SourceFile
    // to an lsproto.Position using ECMA line/character position.
    pub fn byte_pos_to_lsp_position(&self, pos: u32) -> lsproto::Position {
        let (ln, ch) = ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(
            &*self.source_file,
            pos,
        );
        lsproto::Position { line: u32::try_from(ln).unwrap(), character: ch }
    }
}

// go: internal/refactor/context.go:58 RefactorAction 94b4a72aae380c88
// RefactorAction describes a single refactoring action to produce.
//
// Rust: `run` is called once (Go's closure), so it may move what it captures.
pub struct RefactorAction<'f> {
    pub description: String,
    pub run: Box<dyn FnOnce(&mut Tracker) + 'f>,
}

impl Context<'_, '_> {
    // go: internal/refactor/context.go:66 Context.NewRefactorAction be5b5bd7e6efb5a1
    // NewRefactorAction creates a tracker, runs the action's edit closure, and returns
    // a *ls.CodeAction wrapping the resulting edits for the current SourceFile.
    // Returns nil if the closure produced no edits.
    pub fn new_refactor_action(&self, action: RefactorAction<'_>) -> Option<ls::CodeAction> {
        let raw_tracker = change::new_tracker(
            &self.context,
            &self.program.options(),
            self.ls.format_options(),
            // Go's shim accessor `ls.LanguageService_converters` (tsrs patch 100).
            Arc::clone(&self.ls.converters),
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

impl<'c> Context<'_, 'c> {
    /// Rust-only: `new_refactor_action` for an edit closure that also uses the type parser.
    ///
    /// Go's `Run` closures may capture `ctx.TypeParser` (`structuralTypeToSchema`, `writeTagClassAccessors`); here
    /// the closure cannot borrow the parser while `new_refactor_action` borrows the context, so it receives the
    /// parser as an argument. Everything else is `new_refactor_action`'s.
    pub fn new_refactor_action_with_type_parser(
        &mut self,
        description: String,
        run: impl FnOnce(&mut Tracker, &mut TypeParser<'c>),
    ) -> Option<ls::CodeAction> {
        let raw_tracker = change::new_tracker(
            &self.context,
            &self.program.options(),
            self.ls.format_options(),
            // Go's shim accessor `ls.LanguageService_converters` (tsrs patch 100).
            Arc::clone(&self.ls.converters),
        );
        let mut tracker = ets_rewriter::new_tracker(raw_tracker);
        run(&mut tracker, &mut self.type_parser);
        let edits =
            tracker.get_changes().shift_remove(self.source_file.file_name()).unwrap_or_default();
        if edits.is_empty() {
            return None;
        }
        Some(ls::CodeAction { description, changes: edits, ..Default::default() })
    }
}
