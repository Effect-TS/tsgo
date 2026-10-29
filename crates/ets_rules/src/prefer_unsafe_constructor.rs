use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::{Checker, Signature, SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner::get_source_text_of_node_from_source_file;
use ets_typeparser::{TypeParser, get_symbol_if_same_reference, get_symbol_of_declaration};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::X_0_STARTS_A_FIBER_TO_RUN_A_PURE_CONSTRUCTOR_USE_THE_SYNCHRONOUS_VARIANT_1_INSTEAD_EFFECT_PREFER_UNSAFE_CONSTRUCTOR;

// go: internal/rules/prefer_unsafe_constructor.go:19 PreferUnsafeConstructor dddda942f9e0c559
// PreferUnsafeConstructor detects Effect.runSync applied directly to an effect-package
// constructor call whose module also exports a `<name>Unsafe` sibling producing the same
// value synchronously (e.g. Effect.runSync(Scope.make()) -> Scope.makeUnsafe()).
pub static PREFER_UNSAFE_CONSTRUCTOR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferUnsafeConstructor",
    group: "antipattern",
    description: "Suggests replacing Effect.runSync of a pure effect constructor with the synchronous *Unsafe variant exported by the same module",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_prefer_unsafe_constructor(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let original_text =
                get_source_text_of_node_from_source_file(m.source_file, m.outer_callee, false)
                    + "("
                    + &get_source_text_of_node_from_source_file(
                        m.source_file,
                        m.inner_callee,
                        false,
                    )
                    + "(...))";
            let unsafe_text = m.unsafe_callee_text.clone() + "(...)";
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&original_text, &unsafe_text],
            ));
        }
        diags
    },
});

// go: internal/rules/prefer_unsafe_constructor.go:42 PreferUnsafeConstructorMatch ff1c2be4bed80e10
// PreferUnsafeConstructorMatch holds the nodes needed by the diagnostic and quick fix.
#[derive(Clone)]
pub struct PreferUnsafeConstructorMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // Error range on the outer Effect.runSync call
    pub outer_call: P<Node>,        // The Effect.runSync(...) call expression
    pub outer_callee: P<Node>,      // The Effect.runSync callee expression
    pub inner_call: P<Node>,        // The constructor call expression (e.g. Scope.make())
    pub inner_callee: P<Node>,      // The constructor callee expression (e.g. Scope.make)
    pub inner_callee_name: P<Node>, // The identifier holding the constructor name (e.g. make)
    pub unsafe_name: String,        // The sibling export name (e.g. makeUnsafe)
    pub unsafe_callee_text: String, // The callee text with the name replaced (e.g. Scope.makeUnsafe)
}

// go: internal/rules/prefer_unsafe_constructor.go:56 AnalyzePreferUnsafeConstructor b6fa777b7cb1b465
// AnalyzePreferUnsafeConstructor finds Effect.runSync calls whose single argument is a
// direct call to an effect-package constructor with a matching `<name>Unsafe` sibling export.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_prefer_unsafe_constructor(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<PreferUnsafeConstructorMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<PreferUnsafeConstructorMatch>,
    ) -> bool {
        if n.kind() == Kind::CallExpression
            && let Some(m) = analyze_prefer_unsafe_constructor_node(tp, sf, n)
        {
            matches.push(m);
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/prefer_unsafe_constructor.go:81 analyzePreferUnsafeConstructorNode dc0d973c6645b1ac
// analyzePreferUnsafeConstructorNode checks a single call expression for the
// Effect.runSync(<effect constructor>()) pattern with a matching *Unsafe sibling.
//
// Rust: `(match, ok)` is an `Option`; the checker is the parser's.
pub(crate) fn analyze_prefer_unsafe_constructor_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<PreferUnsafeConstructorMatch> {
    let call = node.as_call_expression();
    let call_arguments = call.arguments().nodes();
    if call_arguments.len() != 1 {
        return None;
    }
    // The argument must itself be a direct constructor call, not a variable or composed effect.
    let inner = ast::skip_parentheses(call_arguments[0]);
    if inner.kind() != Kind::CallExpression {
        return None;
    }
    let inner_call = inner.as_call_expression();
    let inner_callee = inner_call.expression();

    let name_node = match inner_callee.kind() {
        Kind::Identifier => inner_callee,
        Kind::PropertyAccessExpression => inner_callee.as_property_access_expression().name(),
        _ => return None,
    };
    if name_node.kind() != Kind::Identifier {
        return None;
    }

    // Spread arguments make the argument list impossible to validate statically.
    for &arg in inner_call.arguments().nodes() {
        if arg.kind() == Kind::SpreadElement {
            return None;
        }
    }

    if !tp.is_node_reference_to_effect_module_api(call.expression(), "runSync") {
        return None;
    }

    // The constructor call must produce an `Effect<A, never, never>`: a fallible or
    // service-requiring effect has no behavior-preserving synchronous replacement.
    let eff = tp.effect_type_at_location(inner)?;
    if !eff.e().flags().intersects(TypeFlags::Never) {
        return None;
    }
    if !eff.r().flags().intersects(TypeFlags::Never) {
        return None;
    }

    let sym = tp.reference_symbol_at_node(inner_callee)?;
    // Use the resolved export name: a named import may alias the local identifier
    // (e.g. `import { make as makeScope }`), and the sibling lives under the export name.
    let name = sym.name();
    if name.is_empty() || name.ends_with("Unsafe") {
        return None;
    }

    let unsafe_name = name.to_string() + "Unsafe";
    for &decl in sym.declarations() {
        let Some(decl_sf) = ast::get_source_file_of_node(decl) else {
            continue;
        };
        if !decl_sf.is_declaration_file() || !tp.is_source_file_in_package(decl_sf, "effect") {
            continue;
        }
        let Some(module_sym) = get_symbol_of_declaration(tp.checker(), decl_sf.as_node()) else {
            continue;
        };
        // The callee must be the module-level export of that name, not a nested member
        // (e.g. a method) that happens to be declared in an effect declaration file.
        let export_member =
            tp.checker().try_get_member_in_module_exports_and_properties(name, module_sym);
        let export_sym = resolve_aliased_symbol(tp.checker(), export_member);
        if get_symbol_if_same_reference(tp.checker(), export_sym, Some(sym)).is_none() {
            continue;
        }
        let sibling_member =
            tp.checker().try_get_member_in_module_exports_and_properties(&unsafe_name, module_sym);
        let Some(sibling) = resolve_aliased_symbol(tp.checker(), sibling_member) else {
            continue;
        };
        if !unsafe_sibling_matches_call(tp, inner, sibling, eff.a()) {
            continue;
        }

        // For a property access like `Scope.make` the sibling stays reachable through the
        // same object, so `Scope.makeUnsafe` both renders in the message and drives the fix.
        // A bare identifier callee has no local binding for the sibling, so only the plain
        // export name is shown and no rename-based fix is possible.
        let mut unsafe_callee_text = unsafe_name.clone();
        if inner_callee.kind() == Kind::PropertyAccessExpression {
            let callee_text = get_source_text_of_node_from_source_file(sf, inner_callee, false);
            let name_text = name_node.text();
            unsafe_callee_text =
                callee_text.strip_suffix(name_text).unwrap_or(&callee_text).to_string()
                    + &unsafe_name;
        }
        return Some(PreferUnsafeConstructorMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, node),
            outer_call: node,
            outer_callee: call.expression(),
            inner_call: inner,
            inner_callee,
            inner_callee_name: name_node,
            unsafe_name,
            unsafe_callee_text,
        });
    }

    None
}

// go: internal/rules/prefer_unsafe_constructor.go:200 unsafeSiblingMatchesCall 0b166b980c642a72
// unsafeSiblingMatchesCall reports whether some call signature of the sibling symbol is
// applicable to the constructor call's exact argument list and produces a value usable
// where the runSync result flows, so the rewrite stays well typed.
//
// Rust: Go's `*ast.CallExpression` is the call node; the checker is the parser's.
pub(crate) fn unsafe_sibling_matches_call(
    tp: &mut TypeParser<'_>,
    inner_call: P<Node>,
    sibling: P<Symbol>,
    success_type: P<Type>,
) -> bool {
    let sibling_type = tp.checker().get_type_of_symbol_exported(sibling);

    let resolved = tp.checker().get_resolved_signature_exported(inner_call);

    let args = inner_call.as_call_expression().arguments().nodes();

    let signatures = tp.checker().get_signatures_of_type(sibling_type, SignatureKind::Call);
    for &sig in signatures {
        if !signature_accepts_arguments(tp, sig, args) {
            continue;
        }
        // Relate the applicable signature alone: calling the sibling where
        // `(constructor params) => <success type>` is expected. This instantiates
        // generic siblings against the concrete argument and result types.
        let c = tp.checker();
        let return_type = c.get_return_type_of_signature(sig);
        let sig_fn_type = c.new_function_type(
            sig.type_parameters(),
            sig.this_parameter(),
            sig.parameters(),
            return_type,
        );
        let expected = c.new_function_type(&[], None, resolved.parameters(), success_type);
        if !c.is_type_assignable_to(sig_fn_type, expected) {
            continue;
        }
        // Concrete returns must also be assignable in the other direction so the
        // rewrite cannot change the expression's inferred type. Generic siblings are
        // accepted on the relation above alone: every *Unsafe sibling in the pinned
        // effect package mirrors its constructor's instantiation, and the real
        // exceptions are all rejected by the argument or relation checks.
        if sig.type_parameters().is_empty() {
            let return_type = c.get_return_type_of_signature(sig);
            if !returns_mutually_assignable(c, Some(return_type), success_type) {
                continue;
            }
        }
        return true;
    }
    false
}

// go: internal/rules/prefer_unsafe_constructor.go:244 signatureAcceptsArguments 8eea3e87e37e40b2
// signatureAcceptsArguments reports whether the signature is applicable to the exact
// argument list of the constructor call: compatible arity and every argument type
// accepted by its parameter (falling back to the constraint for type parameters).
pub(crate) fn signature_accepts_arguments(
    tp: &mut TypeParser<'_>,
    sig: P<Signature>,
    args: &[P<Node>],
) -> bool {
    let params = sig.parameters();
    if args.len() < sig.min_argument_count() {
        return false;
    }
    if args.len() > params.len() && !sig.has_rest_parameter() {
        return false;
    }
    for (i, &arg) in args.iter().enumerate() {
        if sig.has_rest_parameter() && i as i64 >= params.len() as i64 - 1 {
            // Rest arguments are constrained by the whole-signature relation instead.
            break;
        }
        if i >= params.len() {
            return false;
        }
        let param_type = tp.checker().get_type_of_symbol_exported(params[i]);
        let Some(arg_type) = tp.get_type_at_location(arg) else {
            return false;
        };
        if !type_accepts_argument_value(tp.checker(), arg_type, param_type) {
            return false;
        }
    }
    true
}

// go: internal/rules/prefer_unsafe_constructor.go:271 typeAcceptsArgumentValue 9f45878e381de9e5
// typeAcceptsArgumentValue reports whether a value of argType can be passed where
// paramType is expected, treating unconstrained type parameters as accepting anything.
pub(crate) fn type_accepts_argument_value(
    c: &mut Checker,
    arg_type: P<Type>,
    param_type: P<Type>,
) -> bool {
    if param_type.flags().intersects(TypeFlags::TypeParameter) {
        let constraint = c.get_constraint_of_type_parameter(param_type);
        return constraint
            .is_none_or(|constraint| type_accepts_argument_value(c, arg_type, constraint));
    }
    c.is_type_assignable_to(arg_type, param_type)
}

// go: internal/rules/prefer_unsafe_constructor.go:282 returnsMutuallyAssignable 8a9304aa907e79cd
// returnsMutuallyAssignable reports whether a concrete sibling return type and the
// Effect success type denote the same value type: assignable in both directions, so
// the rewrite cannot narrow or widen the expression's inferred type.
pub(crate) fn returns_mutually_assignable(
    c: &mut Checker,
    ret: Option<P<Type>>,
    success_type: P<Type>,
) -> bool {
    let Some(ret) = ret else {
        return false;
    };
    c.is_type_assignable_to(ret, success_type) && c.is_type_assignable_to(success_type, ret)
}

// go: internal/rules/prefer_unsafe_constructor.go:291 resolveAliasedSymbol c41f366fc52c3d7c
// resolveAliasedSymbol follows import/export aliases to the original symbol.
pub(crate) fn resolve_aliased_symbol(c: &mut Checker, sym: Option<P<Symbol>>) -> Option<P<Symbol>> {
    let mut sym = sym;
    while let Some(s) = sym
        && s.flags().intersects(SymbolFlags::Alias)
    {
        sym = Some(c.get_aliased_symbol(s));
    }
    sym
}
