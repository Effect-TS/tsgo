use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_LAYER_CONSTRUCTION_LEAVES_SCOPE_IN_THE_REQUIREMENT_SET_THE_SCOPED_API_REMOVES_SCOPE_FROM_THE_RESULTING_REQUIREMENTS_EFFECT_SCOPE_IN_LAYER_EFFECT;

// go: internal/rules/scope_in_layer_effect.go:18 ScopeInLayerEffect
// ScopeInLayerEffect suggests using Layer.scoped instead of Layer.effect when
// Scope is detected in the layer's requirements.
pub static SCOPE_IN_LAYER_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "scopeInLayerEffect",
    group: "antipattern",
    description: "Suggests using Layer.scoped instead of Layer.effect when Scope is in requirements",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_scope_in_layer_effect(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/scope_in_layer_effect.go:37 ScopeInLayerEffectMatch
// ScopeInLayerEffectMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the scopeInLayerEffect pattern.
#[derive(Clone)]
pub struct ScopeInLayerEffectMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // The pre-computed error range for this match
    pub callee: Option<P<Node>>, // The Layer constructor callee; nil for class declaration matches
}

// go: internal/rules/scope_in_layer_effect.go:45 AnalyzeScopeInLayerEffect
// AnalyzeScopeInLayerEffect finds all Layer.effect*() calls and class declarations
// with Default layer properties where Scope is in the layer's requirements.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_scope_in_layer_effect(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ScopeInLayerEffectMatch> {
    // V3-only rule
    if tp.supported_effect_version() != EffectMajorVersion::V3 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // Stack-based traversal
    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        // Pattern 1: Layer.effect*() calls
        if node.kind() == Kind::CallExpression
            && let Some(m) = match_layer_effect_call(tp, sf, node)
        {
            matches.push(m);
            continue; // skip children
        }

        // Pattern 2: Class declarations with Default layer property
        if node.kind() == Kind::ClassDeclaration
            && let Some(m) = match_class_with_default_layer(tp, sf, node)
        {
            matches.push(m);
            continue; // skip children
        }

        // Enqueue children
        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/scope_in_layer_effect.go:89 matchLayerEffectCall
// matchLayerEffectCall checks if a call expression is Layer.effect*() with Scope in RIn.
pub(crate) fn match_layer_effect_call(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<ScopeInLayerEffectMatch> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();
    let expression = call.expression();

    // Verify this references one of the Layer.effect* constructors from the
    // "effect" package. Symbol resolution also recognizes named imports and
    // stable local aliases whose source spelling does not reveal the API name.
    let mut method_name = "";
    for candidate in ["effect", "effectDiscard", "effectContext"] {
        if tp.is_node_reference_to_effect_layer_module_api(expression, candidate) {
            method_name = candidate;
            break;
        }
    }
    if method_name.is_empty() {
        return None;
    }

    // Get the return type of the call
    let t = tp.get_type_at_location(node)?;

    // Parse as Layer type
    let layer = tp.layer_type(t)?;

    // Check if RIn contains a Scope type
    if !has_scope(tp, layer.r_in()) {
        return None;
    }

    Some(ScopeInLayerEffectMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        callee: Some(expression),
    })
}

// go: internal/rules/scope_in_layer_effect.go:137 matchClassWithDefaultLayer
// matchClassWithDefaultLayer checks if a class declaration has a Default layer property with Scope in RIn.
pub(crate) fn match_class_with_default_layer(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<ScopeInLayerEffectMatch> {
    if node.kind() != Kind::ClassDeclaration {
        return None;
    }
    let class_decl = node.as_class_declaration();

    // Must have a name and heritage clauses
    let name = node.name()?;
    class_decl.heritage_clauses()?;

    // Get the class symbol
    let class_sym = tp.get_symbol_at_location(name)?;

    // Get the class type
    let class_type = tp.checker().get_type_of_symbol_at_location(class_sym, Some(node))?;

    // Check for a "Default" property
    let default_prop = tp.checker().get_property_of_type(class_type, "Default")?;

    // Get the Default property's type
    let default_type = tp.checker().get_type_of_symbol_at_location(default_prop, Some(node))?;

    // Parse as Layer type
    let layer = tp.layer_type(default_type)?;

    // Check if RIn contains a Scope type
    if !has_scope(tp, layer.r_in()) {
        return None;
    }

    Some(ScopeInLayerEffectMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        callee: None,
    })
}

// go: internal/rules/scope_in_layer_effect.go:190 hasScope
// hasScope checks if any union member of the given type is a Scope type.
pub(crate) fn has_scope(tp: &mut TypeParser<'_>, t: P<Type>) -> bool {
    let members = tp.unroll_union_members(t).unwrap_or_default();
    members.into_iter().any(|member| tp.is_scope_type(member))
}
