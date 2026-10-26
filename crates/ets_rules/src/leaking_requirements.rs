use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, FindAncestorResult, Kind, Node, SymbolFlags};
use ets_shim::checker::{Checker, SignatureKind, Type, TypeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::METHODS_OF_THIS_SERVICE_REQUIRE_0_FROM_EVERY_CALLER_THE_REQUIREMENT_BECOMES_PART_OF_THE_PUBLIC_SERVICE_SURFACE_INSTEAD_OF_REMAINING_INTERNAL_TO_LAYER_IMPLEMENTATION_RESOLVE_THESE_DEPENDENCIES_AT_LAYER_CREATION_AND_PROVIDE_THEM_TO_EACH_METHOD_SO_THE_SERVICE_S_TYPE_REFLECTS_ITS_PURPOSE_NOT_ITS_IMPLEMENTATION_TO_SUPPRESS_THIS_DIAGNOSTIC_FOR_SPECIFIC_DEPENDENCY_TYPES_THAT_ARE_INTENTIONALLY_PASSED_THROUGH_E_G_HTTP_SERVER_REQUEST_ADD_EFFECT_LEAKABLE_SERVICE_JSDOC_TO_THEIR_INTERFACE_DECLARATIONS_OR_TO_THIS_SERVICE_BY_ADDING_A_EFFECT_EXPECT_LEAKING_0_JSDOC_MORE_INFO_AND_EXAMPLES_AT_HTTPS_COLON_SLASH_SLASHEFFECT_WEBSITE_SLASHDOCS_SLASHREQUIREMENTS_MANAGEMENT_SLASHLAYERS_SLASH_AVOIDING_REQUIREMENT_LEAKAGE_EFFECT_LEAKING_REQUIREMENTS;

// go: internal/rules/leaking_requirements.go:22 LeakingRequirements
// LeakingRequirements detects when service methods inadvertently expose implementation
// dependencies (Requirements) in their public type signatures. When every method of a
// service requires the same dependency types from callers, this is typically a sign that
// those dependencies should be resolved at Layer creation time instead.
// Supports both V3 and V4, default severity suggestion.
pub static LEAKING_REQUIREMENTS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "leakingRequirements",
    group: "antipattern",
    description: "Detects implementation services leaked in service methods",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Stack-based traversal
        let mut node_to_visit: Vec<P<Node>> = Vec::new();
        ctx.source_file.as_node().for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });

        while let Some(node) = node_to_visit.pop() {
            // Collect types to check and the node to report on
            struct TypeToCheck {
                t: P<Type>,
                report_node: P<Node>,
            }
            let mut types_to_check: Vec<TypeToCheck> = Vec::new();

            match node.kind() {
                Kind::CallExpression => {
                    let call = node.as_call_expression();
                    if call.expression().kind() == Kind::PropertyAccessExpression {
                        let prop_access = call.expression().as_property_access_expression();
                        if prop_access.name().kind() == Kind::Identifier {
                            let name = ets_shim::scanner::get_text_of_node(prop_access.name());
                            if (name == "GenericTag" || name == "Service")
                                && let Some(node_type) = ctx.type_parser.get_type_at_location(node)
                            {
                                types_to_check
                                    .push(TypeToCheck { t: node_type, report_node: node });
                            }
                        }
                    }
                }
                Kind::ClassDeclaration => {
                    if let Some(name) = node.name()
                        && node.as_class_declaration().heritage_clauses().is_some()
                        && let Some(class_sym) = ctx.type_parser.get_symbol_at_location(name)
                        && let Some(class_type) =
                            ctx.checker().get_type_of_symbol_at_location(class_sym, Some(node))
                    {
                        types_to_check.push(TypeToCheck { t: class_type, report_node: name });
                    }
                }
                _ => {}
            }

            if types_to_check.is_empty() {
                // No patterns matched, enqueue children
                node.for_each_child(&mut |child| {
                    node_to_visit.push(child);
                    false
                });
                continue;
            }

            // Check each collected type
            let mut matched = false;
            for ttc in &types_to_check {
                // Try ContextTag first, fall back to ServiceType
                let mut service = ctx.type_parser.context_tag(ttc.t);
                if service.is_none() {
                    service = ctx.type_parser.service_type(ttc.t);
                }
                let Some(service) = service else {
                    continue;
                };

                let mut leaked =
                    parse_leaked_requirements(ctx.type_parser, service.shape(), Some(node));
                if !leaked.is_empty() {
                    leaked = filter_expected_leaking_requirements(
                        ctx.checker(),
                        Some(ttc.report_node),
                        leaked,
                    );
                }
                if !leaked.is_empty() {
                    matched = true;

                    // Sort deterministically by type name (alphabetical)
                    let c = ctx.checker();
                    ets_go::sort::slice(&mut leaked, |a, b| {
                        c.type_to_string_exported(*a) < c.type_to_string_exported(*b)
                    });

                    // Format as "TypeA | TypeB | TypeC"
                    let mut type_names: Vec<String> = Vec::new();
                    for &t in &leaked {
                        type_names.push(ctx.checker().type_to_string_exported(t));
                    }
                    let formatted = type_names.join(" | ");

                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(ttc.report_node),
                        MESSAGE,
                        &[],
                        &[&formatted],
                    ));
                } else {
                    matched = true;
                }
            }

            if !matched {
                // Type resolution failed for all candidates, continue visiting children
                node.for_each_child(&mut |child| {
                    node_to_visit.push(child);
                    false
                });
            }
        }

        diags
    },
});

// go: internal/rules/leaking_requirements.go:133 filterExpectedLeakingRequirements
pub(crate) fn filter_expected_leaking_requirements(
    c: &mut Checker,
    report_node: Option<P<Node>>,
    leaked: Vec<P<Type>>,
) -> Vec<P<Type>> {
    let Some(report_node) = report_node else {
        return leaked;
    };
    if leaked.is_empty() {
        return leaked;
    }

    let mut filtered = Vec::with_capacity(leaked.len());
    for leaked_type in leaked {
        let leaked_type_text = c.type_to_string_exported(leaked_type);
        if is_expected_leaking_service_suppressed(Some(report_node), &leaked_type_text) {
            continue;
        }
        filtered.push(leaked_type);
    }

    filtered
}

// go: internal/rules/leaking_requirements.go:152 isExpectedLeakingServiceSuppressed
//
// Rust: Go's checker argument is unused (and never nil here), so it is dropped.
pub(crate) fn is_expected_leaking_service_suppressed(
    start_node: Option<P<Node>>,
    leaked_service_name: &str,
) -> bool {
    let Some(start_node) = start_node else {
        return false;
    };
    if leaked_service_name.is_empty() {
        return false;
    }

    let Some(source_file) = ast::get_source_file_of_node(start_node) else {
        return false;
    };

    ast::find_ancestor_or_quit(start_node, |current| {
        if has_expected_leaking_comment(source_file.text(), go_pos(current), leaked_service_name) {
            return FindAncestorResult::True;
        }

        if ast::is_class_declaration(current)
            || ast::is_variable_statement(current)
            || ast::is_expression_statement(current)
            || ast::is_statement(current)
        {
            return FindAncestorResult::Quit;
        }

        FindAncestorResult::False
    })
    .is_some()
}

// go: internal/rules/leaking_requirements.go:179 hasExpectedLeakingComment
pub(crate) fn has_expected_leaking_comment(
    source_text: &'static str,
    pos: i64,
    leaked_service_name: &str,
) -> bool {
    if source_text.is_empty()
        || leaked_service_name.is_empty()
        || pos < 0
        || pos > source_text.len() as i64
    {
        return false;
    }

    for comment_range in ets_shim::scanner::get_leading_comment_ranges(source_text, pos as u32) {
        let start = comment_range.pos() as usize;
        let end = comment_range.end() as usize;
        if start >= end || end > source_text.len() {
            continue;
        }

        let comment_text = &source_text[start..end];
        for line in comment_text.split('\n') {
            let Some((_, suffix)) = line.split_once("@effect-expect-leaking") else {
                continue;
            };
            if suffix.contains(leaked_service_name) {
                return true;
            }
        }
    }

    false
}

// go: internal/rules/leaking_requirements.go:208 parseLeakedRequirements
// parseLeakedRequirements analyzes the service shape to find requirement types that
// are shared across all effect-typed members. This is the "leaking requirements" heuristic.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn parse_leaked_requirements(
    tp: &mut TypeParser<'_>,
    service_shape: P<Type>,
    at_location: Option<P<Node>>,
) -> Vec<P<Type>> {
    let properties = tp.checker().get_properties_of_type(service_shape);
    if properties.is_empty() {
        return Vec::new();
    }

    let mut memory: FxHashMap<String, P<Type>> = FxHashMap::default();
    let mut shared_requirements_keys: Option<Vec<String>> = None;
    let mut shared_initialized = false;
    let mut effect_members = 0;

    let mut should_exclude = |tp: &mut TypeParser<'_>, t: P<Type>| -> bool {
        // Exclude never
        if t.flags().intersects(TypeFlags::Never) {
            return true;
        }
        // Exclude Scope types
        if tp.is_scope_type(t) {
            return true;
        }
        false
    };

    for &property in properties {
        let Some(service_property_type) =
            tp.checker().get_type_of_symbol_at_location(property, at_location)
        else {
            continue;
        };

        // Try to get the Effect's R type - either directly from the property type
        // or from the return type of a single call signature
        let mut effect_context_type = None;

        if let Some(effect) = tp.effect_type(service_property_type) {
            effect_context_type = Some(effect.r());
        } else {
            // Try call signature: if exactly 1 call signature, parse return type as Effect
            let sigs =
                tp.checker().get_signatures_of_type(service_property_type, SignatureKind::Call);
            if sigs.len() == 1 {
                let ret_type = tp.checker().get_return_type_of_signature(sigs[0]);
                if let Some(ret_effect) = tp.effect_type(ret_type) {
                    effect_context_type = Some(ret_effect.r());
                }
            }
        }

        let Some(effect_context_type) = effect_context_type else {
            continue;
        };

        effect_members += 1;
        let result = tp.append_to_unique_types_map(
            &mut memory,
            Some(effect_context_type),
            Some(&mut should_exclude),
        );

        if !shared_initialized {
            shared_requirements_keys = result.all_indexes().copy();
            shared_initialized = true;
        } else {
            // Intersect with current keys
            shared_requirements_keys = intersect_string_slices(
                ets_readonly::new(shared_requirements_keys.as_deref()),
                result.all_indexes(),
            );
            if shared_requirements_keys.as_ref().is_none_or(Vec::is_empty) {
                return Vec::new();
            }
        }
    }

    // Need at least 2 effect members for the heuristic
    let shared_requirements_keys = shared_requirements_keys.unwrap_or_default();
    if !shared_initialized || shared_requirements_keys.is_empty() || effect_members < 2 {
        return Vec::new();
    }

    // Collect the shared requirement types, filtering out those with @effect-leakable-service
    let mut leaked = Vec::new();
    for key in &shared_requirements_keys {
        let Some(&t) = memory.get(key) else {
            continue;
        };
        if has_leakable_service_annotation(tp.checker(), t) {
            continue;
        }
        leaked.push(t);
    }

    leaked
}

// go: internal/rules/leaking_requirements.go:304 hasLeakableServiceAnnotation
// hasLeakableServiceAnnotation checks if a type's declarations contain the
// @effect-leakable-service JSDoc annotation.
pub(crate) fn has_leakable_service_annotation(c: &mut Checker, t: P<Type>) -> bool {
    let Some(mut sym) = t.symbol() else {
        return false;
    };

    // Resolve aliases
    if sym.flags().intersects(SymbolFlags::Alias) {
        sym = c.get_aliased_symbol(sym);
    }

    for &decl in sym.declarations() {
        let Some(sf) = ast::get_source_file_of_node(decl) else {
            continue;
        };
        let text = sf.text();
        let start = go_pos(decl);
        let end = go_end(decl);
        if start < 0 || end < 0 || start >= end || end > text.len() as i64 {
            continue;
        }
        let snippet =
            ets_shim::core::stringutil::go_strings_to_lower(&text[start as usize..end as usize]);
        if snippet.contains("@effect-leakable-service") {
            return true;
        }
    }

    false
}

// go: internal/rules/leaking_requirements.go:342 intersectStringSlices
// intersectStringSlices returns the elements that appear in both slices.
//
// Rust: the result owns its elements (Go returns a read-only view of a fresh slice); `None` is Go's nil slice.
pub(crate) fn intersect_string_slices(
    a: Slice<'_, String>,
    b: Slice<'_, String>,
) -> Option<Vec<String>> {
    let mut set: FxHashSet<&str> = FxHashSet::default();
    for (_, s) in b.all() {
        set.insert(s);
    }
    let mut result: Option<Vec<String>> = None;
    for (_, s) in a.all() {
        if set.contains(s.as_str()) {
            result.get_or_insert_with(Vec::new).push(s.clone());
        }
    }
    result
}
