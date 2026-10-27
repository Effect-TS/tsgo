use ets_etscore::{ResolvedEffectPluginOptions, Severity};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::checker::{Checker, Program};
use ets_shim::core::TextRange;
use ets_shim::core::context;
use ets_shim::diagnostics::Message;
use ets_typeparser::TypeParser;

// go: internal/rule/context.go:17 Context bac8a1063c911107
// Context bundles the checker, source file, and default severity for a rule invocation.
// It provides a NewDiagnostic helper that simplifies diagnostic creation.
//
// Rust: Go's `Checker` and `TypeParser` fields point at the same checker; `ets_typeparser::TypeParser` borrows the
// checker mutably, so the context holds the parser (shared by every rule of a run, as Go shares the pointer) and
// `checker()` reaches the checker through it. `Program` is the checker's program (`checker.Program`).
pub struct Context<'a, 'c> {
    pub context: &'a context::Context,
    pub program: &'static dyn Program,
    pub type_parser: &'a mut TypeParser<'c>,
    pub source_file: P<SourceFile>,
    pub options: Option<&'a ResolvedEffectPluginOptions>,
    pub(crate) default_severity: Severity,
}

impl Context<'_, '_> {
    /// Rust-only: Go's `ctx.Checker` (see `Context`).
    pub fn checker(&mut self) -> &mut Checker {
        self.type_parser.checker()
    }
}

// go: internal/rule/context.go:28 NewContext 896d2711b0b0a4ad
// NewContext creates a new Context for a rule invocation.
//
// Rust: the checker comes with the type parser (see `Context`).
pub fn new_context<'a, 'c>(
    ctx: &'a context::Context,
    program: &'static dyn Program,
    tp: &'a mut TypeParser<'c>,
    sf: P<SourceFile>,
    options: Option<&'a ResolvedEffectPluginOptions>,
    default_severity: Severity,
) -> Context<'a, 'c> {
    Context { context: ctx, program, type_parser: tp, source_file: sf, options, default_severity }
}

impl Context<'_, '_> {
    // go: internal/rule/context.go:42 Context.GetErrorRange 490e24f06c1be4a5
    // GetErrorRange computes the error range for a node in the context's source file.
    // Use this in rules that don't have Analyze functions to get a location before calling NewDiagnostic.
    pub fn get_error_range(&self, node: P<Node>) -> TextRange {
        ets_shim::scanner::get_error_range_for_node(self.source_file, node)
    }

    // go: internal/rule/context.go:49 Context.NewDiagnostic 54c8ae3be9ee3227
    // NewDiagnostic creates a diagnostic using the context's source file and default severity.
    // The loc is the pre-computed error range, message provides the diagnostic code and key,
    // relatedInformation can be nil, and args are variadic message format arguments.
    pub fn new_diagnostic(
        &self,
        sf: P<SourceFile>,
        loc: TextRange,
        message: &'static Message,
        related_information: &[P<Diagnostic>],
        args: &[&str],
    ) -> P<Diagnostic> {
        let mut message_args = Vec::new();
        if !args.is_empty() {
            message_args = args.iter().map(|arg| (*arg).to_string()).collect();
        }
        ets_shim::ast::new_diagnostic_from_serialized(
            Some(sf),
            loc,
            message.code(),
            ets_directives::to_category(self.default_severity),
            message.key(),
            message_args,
            &[],
            related_information,
            false,
            false,
            false,
        )
    }
}
