use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::SignatureKind;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::This_class_extends_the_native_Error_type_directly_Untagged_native_errors_lose_distinction_in_the_Effect_failure_channel_effect_extendsNativeError;

// go: internal/rules/extends_native_error.go:14 ExtendsNativeError
pub static EXTENDS_NATIVE_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "extendsNativeError",
    group: "effectNative",
    description: "Warns when a class directly extends the native Error class",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_extends_native_error(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/extends_native_error.go:31 ExtendsNativeErrorMatch
#[derive(Clone)]
pub struct ExtendsNativeErrorMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
}

// go: internal/rules/extends_native_error.go:36 AnalyzeExtendsNativeError
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_extends_native_error(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ExtendsNativeErrorMatch> {
    let Some(error_symbol) =
        tp.checker().resolve_name_exported("Error", None, SymbolFlags::Type, false)
    else {
        return Vec::new();
    };

    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::ClassDeclaration {
            if let Some(m) = check_extends_native_error(tp, sf, node, error_symbol) {
                matches.push(m);
            }
        }

        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/extends_native_error.go:67 checkExtendsNativeError
pub(crate) fn check_extends_native_error(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
    error_symbol: P<Symbol>,
) -> Option<ExtendsNativeErrorMatch> {
    let extends_elements = ast::get_extends_heritage_clause_elements(node);
    if extends_elements.is_empty() {
        return None;
    }

    for &elem in extends_elements {
        if elem.kind() != Kind::ExpressionWithTypeArguments {
            continue;
        }
        let expr = elem.as_expression_with_type_arguments().expression();

        let expr_symbol = tp.get_symbol_at_location(expr);
        let mut resolved_symbol = expr_symbol;
        if let Some(symbol) = resolved_symbol
            && symbol.flags().intersects(SymbolFlags::Alias)
        {
            resolved_symbol = Some(tp.checker().get_aliased_symbol(symbol));
        }

        let mut is_native_error = resolved_symbol == Some(error_symbol);
        if !is_native_error && resolved_symbol.is_some() && resolved_symbol != Some(error_symbol) {
            if let Some(expr_type) = tp.get_type_at_location(expr) {
                let construct_signatures = tp
                    .checker()
                    .get_signatures_of_type_exported(expr_type, SignatureKind::Construct);
                if !construct_signatures.is_empty() {
                    let instance_type =
                        tp.checker().get_return_type_of_signature(construct_signatures[0]);
                    if instance_type.symbol() == Some(error_symbol) {
                        is_native_error = true;
                    }
                }
            }
        }

        if is_native_error {
            let location_node = node.name().unwrap_or(expr);
            return Some(ExtendsNativeErrorMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, location_node),
            });
        }
    }

    None
}
