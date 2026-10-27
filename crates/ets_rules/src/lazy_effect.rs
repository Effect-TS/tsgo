use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/lazy_effect.go:16 LazyEffect e0fe2932415be719
pub static LAZY_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "lazyEffect",
    group: "antipattern",
    description: "Suggests avoiding exported zero-argument functions and service members that lazily return Effect or Stream values",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![
        tsdiag::X_0_RETURNS_A_LAZY_1_1_IS_ALREADY_LAZY_SO_WRAPPING_IT_IN_A_ZERO_ARGUMENT_FUNCTION_ADDS_UNNECESSARY_INDIRECTION_EFFECT_LAZY_EFFECT.code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        if !ctx.type_parser.detect_effect_version_string().starts_with("4.") {
            return Vec::new();
        }

        let mut diags = Vec::new();

        for &stmt in ctx.source_file.statements().nodes() {
            diags.extend(check_lazy_effect_export(ctx, stmt));
            diags.extend(check_lazy_effect_interface(ctx, stmt));
            diags.extend(check_lazy_effect_service(ctx, stmt));
        }

        diags
    },
}
});

// go: internal/rules/lazy_effect.go:46 checkLazyEffectInterface 8500b5a3363255ec
pub(crate) fn check_lazy_effect_interface(
    ctx: &mut Context<'_, '_>,
    stmt: P<Node>,
) -> Vec<P<Diagnostic>> {
    if stmt.kind() != Kind::InterfaceDeclaration
        || !ast::has_syntactic_modifier(stmt, ModifierFlags::Export)
    {
        return Vec::new();
    }
    let Some(stmt_name) = stmt.name() else {
        return Vec::new();
    };

    let interface_name = ets_shim::scanner::get_text_of_node(stmt_name);
    let mut diags = Vec::new();
    for &member in stmt.as_interface_declaration().members().nodes() {
        let Some(member_name) = member.name() else {
            continue;
        };
        if member.kind() != Kind::PropertySignature && member.kind() != Kind::MethodSignature {
            continue;
        }

        let member_type = ctx.type_parser.get_type_at_location(member_name);
        let Some(lazy_type_name) = lazy_effect_like_type_name(ctx.type_parser, member_type) else {
            continue;
        };

        let message_subject = format!(
            "Interface '{}' member '{}'",
            interface_name,
            ets_shim::scanner::get_text_of_node(member_name)
        );
        diags.push(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(member_name),
            &tsdiag::X_0_RETURNS_A_LAZY_1_1_IS_ALREADY_LAZY_SO_WRAPPING_IT_IN_A_ZERO_ARGUMENT_FUNCTION_ADDS_UNNECESSARY_INDIRECTION_EFFECT_LAZY_EFFECT,
            &[],
            &[&message_subject, lazy_type_name],
        ));
    }

    diags
}

// go: internal/rules/lazy_effect.go:81 checkLazyEffectExport 86872e6c6d3748ad
pub(crate) fn check_lazy_effect_export(
    ctx: &mut Context<'_, '_>,
    stmt: P<Node>,
) -> Vec<P<Diagnostic>> {
    if !ast::has_syntactic_modifier(stmt, ModifierFlags::Export) {
        return Vec::new();
    }

    match stmt.kind() {
        Kind::FunctionDeclaration => {
            let Some(name) = stmt.name() else {
                return Vec::new();
            };
            if stmt.body().is_none() || name.kind() != Kind::Identifier {
                return Vec::new();
            }
            lazy_effect_diagnostic_for_exported_declaration(ctx, name)
        }

        Kind::VariableStatement => {
            let decl_list = stmt.as_variable_statement().declaration_list();

            let mut diags = Vec::new();
            for &decl in decl_list.as_variable_declaration_list().declarations().nodes() {
                let Some(name) = decl.name() else {
                    continue;
                };
                if name.kind() != Kind::Identifier {
                    continue;
                }
                diags.extend(lazy_effect_diagnostic_for_exported_declaration(ctx, name));
            }
            diags
        }

        _ => Vec::new(),
    }
}

// go: internal/rules/lazy_effect.go:112 lazyEffectDiagnosticForExportedDeclaration 4c0142e0d83d9131
pub(crate) fn lazy_effect_diagnostic_for_exported_declaration(
    ctx: &mut Context<'_, '_>,
    name: P<Node>,
) -> Vec<P<Diagnostic>> {
    let decl_type = ctx.type_parser.get_type_at_location(name);
    let Some(lazy_type_name) = lazy_effect_like_type_name(ctx.type_parser, decl_type) else {
        return Vec::new();
    };

    let message_subject =
        format!("Exported declaration '{}'", ets_shim::scanner::get_text_of_node(name));
    vec![ctx.new_diagnostic(
        ctx.source_file,
        ctx.get_error_range(name),
        &tsdiag::X_0_RETURNS_A_LAZY_1_1_IS_ALREADY_LAZY_SO_WRAPPING_IT_IN_A_ZERO_ARGUMENT_FUNCTION_ADDS_UNNECESSARY_INDIRECTION_EFFECT_LAZY_EFFECT,
        &[],
        &[&message_subject, lazy_type_name],
    )]
}

// go: internal/rules/lazy_effect.go:130 checkLazyEffectService e000fdf6796d6d19
pub(crate) fn check_lazy_effect_service(
    ctx: &mut Context<'_, '_>,
    stmt: P<Node>,
) -> Vec<P<Diagnostic>> {
    if stmt.kind() != Kind::ClassDeclaration {
        return Vec::new();
    }
    let Some(stmt_name) = stmt.name() else {
        return Vec::new();
    };
    if stmt_name.kind() != Kind::Identifier {
        return Vec::new();
    }
    if ctx.type_parser.extends_context_service(stmt).is_none() {
        return Vec::new();
    }

    let Some(class_sym) = ctx.type_parser.get_symbol_at_location(stmt_name) else {
        return Vec::new();
    };
    let Some(class_type) = ctx.checker().get_type_of_symbol_at_location(class_sym, Some(stmt_name))
    else {
        return Vec::new();
    };

    // Rust: a parsed `Service` always has its shape (Go's nil check on `Shape()` drops).
    let Some(service) = ctx.type_parser.service_type(class_type) else {
        return Vec::new();
    };

    let service_name = ets_shim::scanner::get_text_of_node(stmt_name);
    let mut diags = Vec::new();
    for &member in ctx.checker().get_properties_of_type(service.shape()) {
        let member_type = ctx.checker().get_type_of_symbol_at_location(member, Some(stmt_name));
        let Some(lazy_type_name) = lazy_effect_like_type_name(ctx.type_parser, member_type) else {
            continue;
        };

        let message_subject = format!("Service '{}' member '{}'", service_name, member.name());
        diags.push(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(stmt_name),
            &tsdiag::X_0_RETURNS_A_LAZY_1_1_IS_ALREADY_LAZY_SO_WRAPPING_IT_IN_A_ZERO_ARGUMENT_FUNCTION_ADDS_UNNECESSARY_INDIRECTION_EFFECT_LAZY_EFFECT,
            &[],
            &[&message_subject, lazy_type_name],
        ));
    }

    diags
}

// go: internal/rules/lazy_effect.go:179 lazyEffectLikeTypeName 61e85171bab09a03
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); `(name, ok)` is `Option<name>`.
pub(crate) fn lazy_effect_like_type_name(
    tp: &mut TypeParser<'_>,
    t: Option<P<Type>>,
) -> Option<&'static str> {
    let t = t?;

    let call_signatures = tp.checker().get_signatures_of_type(t, SignatureKind::Call);
    if call_signatures.len() != 1 {
        return None;
    }

    let sig = call_signatures[0];
    if !sig.type_parameters().is_empty() {
        return None;
    }
    if !sig.parameters().is_empty() {
        return None;
    }

    let return_type = tp.checker().get_return_type_of_signature(sig);

    if tp.strict_is_effect_type(return_type) {
        return Some("Effect");
    }
    if tp.layer_type(return_type).is_some() {
        return Some("Layer");
    }
    if tp.stream_type(return_type).is_some() {
        return Some("Stream");
    }

    None
}
