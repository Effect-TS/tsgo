use ets_shim::ast::SourceFile;
use ets_shim::checker::Checker;
use ets_shim::compiler::Program;
use ets_shim::core::P;
use ets_shim::core::context::Context as GoContext;
use ets_shim::ls::LanguageService;
use ets_shim::lsproto;
use ets_typeparser::TypeParser;

// go: internal/completion/context.go:16 Context 899ddee2a2d8c4e6
// Context bundles the completion request data and provides helpers for completion implementations.
// It provides access to the source file, cursor position, existing completion items,
//
// Rust: Go holds both `Checker` and `TypeParser`; `ets_typeparser::TypeParser` borrows the checker mutably for its
// lifetime, so the context holds the parser and reaches the checker through it (`checker()`), as
// `ets_fixable::Context` does. The existing items are borrowed (Go shares the slice).
pub struct Context<'a, 'c> {
    pub source_file: P<SourceFile>,
    pub position: u32,
    pub existing_items: &'a [lsproto::CompletionItem],
    pub program: &'static Program,
    pub type_parser: TypeParser<'c>,

    pub context: GoContext,
    #[expect(
        dead_code,
        reason = "Go's unexported field, set by NewContext and read by nothing yet"
    )]
    pub(crate) ls: &'a LanguageService,
}

impl Context<'_, '_> {
    /// Rust-only: Go's `Checker` field (the checker `type_parser` runs on; see the struct).
    pub fn checker(&mut self) -> &mut Checker {
        self.type_parser.checker()
    }
}

// go: internal/completion/context.go:29 NewContext 359def73a2872c19
// NewContext creates a completion Context from the completion callback parameters.
//
// Rust: Go panics on a nil program or checker; neither can be nil here. The checker is `tp`'s (see `Context`).
pub fn new_context<'a, 'c>(
    ctx: &GoContext,
    source_file: P<SourceFile>,
    position: u32,
    existing_items: &'a [lsproto::CompletionItem],
    program: &'static Program,
    lang_service: &'a LanguageService,
    tp: TypeParser<'c>,
) -> Context<'a, 'c> {
    Context {
        source_file,
        position,
        existing_items,
        program,
        type_parser: tp,
        context: ctx.clone(),
        ls: lang_service,
    }
}
