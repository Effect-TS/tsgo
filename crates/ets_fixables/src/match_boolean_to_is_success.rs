use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::P;
use ets_shim::ast::{self, Node, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::ModuleReference;

use crate::*;

/// Rust-only: the fixable's `Effect.isSuccess` message (Go spells it out in `ErrorCodes`).
static IS_SUCCESS_MESSAGE: &Message = &tsdiag::EFFECT_IS_SUCCESS_EXPRESSES_THIS_CHECK_MORE_DIRECTLY_THAN_EFFECT_0_FOLDING_TO_COMPLEMENTARY_BOOLEAN_LITERALS_EFFECT_MATCH_BOOLEAN_TO_IS_SUCCESS;

/// Rust-only: the fixable's `Effect.isFailure` message (Go spells it out in `ErrorCodes`).
static IS_FAILURE_MESSAGE: &Message = &tsdiag::EFFECT_IS_FAILURE_EXPRESSES_THIS_CHECK_MORE_DIRECTLY_THAN_EFFECT_0_FOLDING_TO_COMPLEMENTARY_BOOLEAN_LITERALS_EFFECT_MATCH_BOOLEAN_TO_IS_SUCCESS;

// go: internal/fixables/match_boolean_to_is_success.go:13 MatchBooleanToIsSuccessFix 698a4613178d49b7
pub static MATCH_BOOLEAN_TO_IS_SUCCESS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "matchBooleanToIsSuccess",
    description: "Replace Effect.match boolean folds with Effect.isSuccess or Effect.isFailure",
    error_codes: vec![IS_SUCCESS_MESSAGE.code(), IS_FAILURE_MESSAGE.code()],
    fix_ids: vec!["matchBooleanToIsSuccess_fix"],
    run: run_match_boolean_to_is_success_fix,
});

// go: internal/fixables/match_boolean_to_is_success.go:24 runMatchBooleanToIsSuccessFix f6b99c76f17eacc3
fn run_match_boolean_to_is_success_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_match_boolean_to_is_success(&mut ctx.type_parser, Some(source_file))
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let member = effect_module_member(m.replacement_name);
        let (reference, named_imports) =
            match_boolean_replacement_reference(ctx, m.callee, &member);
        let Some(reference) = reference else {
            continue;
        };

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{}", m.replacement_name),
            run: Box::new(move |tracker| {
                let replacement = reference.expression(tracker, &member);
                let replaced = if m.point_free {
                    // match({...}) becomes the bare isSuccess / isFailure function.
                    tracker.replace_piping_flow_prefix(
                        Some(m.source_file),
                        Some(&m.flow),
                        1,
                        Some(replacement),
                    )
                } else {
                    let transformation = m.flow.transformations().at(m.index).clone();
                    tracker.replace_piping_flow_transformation(
                        Some(m.source_file),
                        Some(&transformation),
                        PipingFlowTransformationReplacement {
                            callee: Some(replacement),
                            type_arguments: None,
                            arguments: None,
                        },
                    )
                };
                if replaced && let Some(named_imports) = named_imports {
                    let elements = named_imports.as_named_imports().elements();
                    let specifier = tracker.new_import_specifier(
                        false,
                        None,
                        tracker.new_identifier(member.name),
                    );
                    tracker.insert_import_specifier_at_index(
                        m.source_file,
                        specifier,
                        named_imports,
                        elements.nodes().len(),
                    );
                }
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/match_boolean_to_is_success.go:64 matchBooleanReplacementReference a9508c4396295719
// matchBooleanReplacementReference keeps the qualifier the match callee is
// written with: Q.match gives Q.isSuccess, and a named import of match gives a
// bare isSuccess from the same import. When the bare name has to be imported, namedImports is the list to
// add it to. Otherwise it falls back to the file's Effect module reference.
fn match_boolean_replacement_reference(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
    member: &ModuleMember,
) -> (Option<ModuleMemberReference>, Option<P<Node>>) {
    if let Some(qualifier) = match_boolean_callee_qualifier(ctx, callee)
        && let Some(reference) = imported_module_member(ctx, callee, &qualifier, member)
    {
        return (Some(reference), None);
    }
    if let (Some(reference), named_imports) = named_import_module_member(ctx, callee, member) {
        return (Some(reference), named_imports);
    }
    let preferred = ets_typeparser::find_effect_module_reference(Some(ctx.source_file));
    (resolve_module_member(ctx, callee, &preferred, member), None)
}

// go: internal/fixables/match_boolean_to_is_success.go:78 matchBooleanCalleeQualifier b1994c16a7eff580
// matchBooleanCalleeQualifier returns the Effect module path of a Q.match
// callee.
//
// Rust: Go's nil `ModuleReference` is `None`.
fn match_boolean_callee_qualifier(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
) -> Option<ModuleReference> {
    let callee = ast::skip_parentheses(callee);
    if !ast::is_property_access_expression(callee) {
        return None;
    }
    let qualifier = callee.expression();
    if !ctx.type_parser.is_expression_effect_module(qualifier) {
        return None;
    }
    identifier_path(qualifier?)
}

// go: internal/fixables/match_boolean_to_is_success.go:92 identifierPath 63de0494736b1d00
// identifierPath returns the names of an identifier or a chain of property
// accesses on identifiers, or nil for any other expression.
//
// Rust: Go's nil `ModuleReference` is `None`.
fn identifier_path(node: P<Node>) -> Option<ModuleReference> {
    if ast::is_identifier(node) {
        return Some(ModuleReference(vec![node.text().to_string()]));
    } else if ast::is_property_access_expression(node)
        && let Some(name) = node.name()
        && ast::is_identifier(name)
        && let Some(path) = node.expression().and_then(identifier_path)
    {
        return Some(path.append(&[name.text()]));
    }
    None
}

// go: internal/fixables/match_boolean_to_is_success.go:108 namedImportModuleMember d91d46b4294c6a15
// namedImportModuleMember handles a callee that is a named import binding of
// the match API: it reuses a bare member binding already in scope, or returns
// the bare member name with the callee's NamedImports to add it to when the
// name is free and the imported module exports it.
fn named_import_module_member(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
    member: &ModuleMember,
) -> (Option<ModuleMemberReference>, Option<P<Node>>) {
    let callee = ast::skip_parentheses(callee);
    if !ast::is_identifier(callee) {
        return (None, None);
    }
    let symbol = ctx.checker().resolve_name_exported(
        callee.text(),
        callee,
        SymbolFlags::Value | SymbolFlags::Alias,
        false,
    );
    let binding = import_binding_name(ctx, callee, symbol);
    let Some(binding_parent) = binding.and_then(|binding| binding.parent()) else {
        return (None, None);
    };
    if !ast::is_import_specifier(binding_parent) {
        return (None, None);
    }
    let Some(named_imports) = binding_parent.parent() else {
        return (None, None);
    };
    let Some(import_declaration) = named_imports.parent().and_then(|clause| clause.parent()) else {
        return (None, None);
    };
    if !ast::is_named_imports(named_imports) || !ast::is_import_declaration(import_declaration) {
        return (None, None);
    }
    let module_symbol = ctx
        .type_parser
        .get_symbol_at_location(import_declaration.as_import_declaration().module_specifier());
    let Some(module_symbol) = module_symbol else {
        return (None, None);
    };
    let export = ctx.checker().try_get_member_in_module_exports(member.name, module_symbol);
    let Some(mut export) = export else {
        return (None, None);
    };
    if export.flags().intersects(SymbolFlags::Alias) {
        export = ctx.checker().get_aliased_symbol(export);
    }
    let export_name = export.value_declaration().and_then(|declaration| declaration.name());
    let Some(export_name) = export_name else {
        return (None, None);
    };
    if !(member.is_member)(&mut ctx.type_parser, export_name) {
        return (None, None);
    }

    let existing = ctx.checker().resolve_name_exported(
        member.name,
        callee,
        SymbolFlags::Value | SymbolFlags::Alias,
        false,
    );
    if existing.is_none() {
        return (
            Some(ModuleMemberReference {
                path: ModuleReference(vec![member.name.to_string()]),
                via_module: false,
            }),
            Some(named_imports),
        );
    }
    if let Some(name) = import_binding_name(ctx, callee, existing)
        && (member.is_member)(&mut ctx.type_parser, name)
    {
        return (
            Some(ModuleMemberReference {
                path: ModuleReference(vec![name.text().to_string()]),
                via_module: false,
            }),
            None,
        );
    }
    (None, None)
}
