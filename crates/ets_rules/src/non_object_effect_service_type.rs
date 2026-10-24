use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::checker::{SignatureKind, Type, TypeFlags};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};

// go: internal/rules/non_object_effect_service_type.go:16 NonObjectEffectServiceType
// NonObjectEffectServiceType checks that Effect.Service option properties
// (succeed, sync, effect, scoped) do not resolve to primitive types.
// V3-only, default severity error.
pub static NON_OBJECT_EFFECT_SERVICE_TYPE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "nonObjectEffectServiceType",
        group: "correctness",
        description: "Ensures Effect.Service types are objects, not primitives",
        default_severity: Severity::Error,
        supported_effect: vec!["v3"],
        codes: vec![
            tsdiag::Effect_Service_is_declared_with_a_primitive_service_type_Effect_Service_models_object_shaped_services_primitive_values_use_Context_Tag_or_Effect_Tag_directly_effect_nonObjectEffectServiceType.code(),
        ],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            // V3-only rule
            if ctx.type_parser.supported_effect_version() != EffectMajorVersion::V3 {
                return Vec::new();
            }

            let mut diags = Vec::new();

            // Stack-based traversal
            let mut node_to_visit: Vec<P<Node>> = Vec::new();
            ctx.source_file.as_node().for_each_child(&mut |child| {
                node_to_visit.push(child);
                false
            });

            while let Some(node) = node_to_visit.pop() {
                if node.kind() == Kind::ClassDeclaration {
                    let d = check_service_property_types(ctx, node);
                    if !d.is_empty() {
                        diags.extend(d);
                        continue; // skip children
                    }
                }

                // Enqueue children
                node.for_each_child(&mut |child| {
                    node_to_visit.push(child);
                    false
                });
            }

            diags
        },
    }
});

// go: internal/rules/non_object_effect_service_type.go:62 checkServicePropertyTypes
// checkServicePropertyTypes checks if a class extending Effect.Service has option
// properties that resolve to primitive types.
pub(crate) fn check_service_property_types(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Vec<P<Diagnostic>> {
    let Some(service_result) = ctx.type_parser.extends_effect_v3_service(node) else {
        return Vec::new();
    };

    let Some(options) = service_result.options() else {
        return Vec::new();
    };
    if options.kind() != Kind::ObjectLiteralExpression {
        return Vec::new();
    }

    let obj_lit = options.as_object_literal_expression();

    let mut diags = Vec::new();

    for &prop in obj_lit.properties.nodes() {
        if prop.kind() != Kind::PropertyAssignment {
            continue;
        }
        let Some(name) = prop.name() else {
            continue;
        };
        if name.kind() != Kind::Identifier {
            continue;
        }

        let property_name = ets_shim::scanner::get_text_of_node(name);
        let Some(initializer) = prop.initializer() else {
            continue;
        };

        match property_name.as_str() {
            "succeed" => {
                let value_type = ctx.type_parser.get_type_at_location(initializer);
                if let Some(value_type) = value_type
                    && is_primitive_type(ctx.type_parser, value_type)
                {
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(name),
                        &tsdiag::Effect_Service_is_declared_with_a_primitive_service_type_Effect_Service_models_object_shaped_services_primitive_values_use_Context_Tag_or_Effect_Tag_directly_effect_nonObjectEffectServiceType,
                        &[],
                        &[],
                    ));
                }
            }

            "sync" => {
                let Some(value_type) = ctx.type_parser.get_type_at_location(initializer) else {
                    continue;
                };
                let signatures =
                    ctx.checker().get_signatures_of_type(value_type, SignatureKind::Call);
                for &sig in signatures {
                    let return_type = ctx.checker().get_return_type_of_signature(sig);
                    if is_primitive_type(ctx.type_parser, return_type) {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ctx.get_error_range(name),
                            &tsdiag::Effect_Service_is_declared_with_a_primitive_service_type_Effect_Service_models_object_shaped_services_primitive_values_use_Context_Tag_or_Effect_Tag_directly_effect_nonObjectEffectServiceType,
                            &[],
                            &[],
                        ));
                        break;
                    }
                }
            }

            "effect" | "scoped" => {
                let Some(value_type) = ctx.type_parser.get_type_at_location(initializer) else {
                    continue;
                };

                // Try direct EffectType parse first
                let effect_result = ctx.type_parser.effect_type(value_type);
                if let Some(effect_result) = effect_result {
                    if is_primitive_type(ctx.type_parser, effect_result.a()) {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ctx.get_error_range(name),
                            &tsdiag::Effect_Service_is_declared_with_a_primitive_service_type_Effect_Service_models_object_shaped_services_primitive_values_use_Context_Tag_or_Effect_Tag_directly_effect_nonObjectEffectServiceType,
                            &[],
                            &[],
                        ));
                    }
                    continue;
                }

                // Fall back to call signatures
                let signatures =
                    ctx.checker().get_signatures_of_type(value_type, SignatureKind::Call);
                for &sig in signatures {
                    let return_type = ctx.checker().get_return_type_of_signature(sig);
                    let effect_return_result = ctx.type_parser.effect_type(return_type);
                    if let Some(effect_return_result) = effect_return_result
                        && is_primitive_type(ctx.type_parser, effect_return_result.a())
                    {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ctx.get_error_range(name),
                            &tsdiag::Effect_Service_is_declared_with_a_primitive_service_type_Effect_Service_models_object_shaped_services_primitive_values_use_Context_Tag_or_Effect_Tag_directly_effect_nonObjectEffectServiceType,
                            &[],
                            &[],
                        ));
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    diags
}

// go: internal/rules/non_object_effect_service_type.go:171 isPrimitiveType
// isPrimitiveType checks if a type (or any member of a union type) is a primitive type.
pub(crate) fn is_primitive_type(tp: &TypeParser<'_>, t: P<Type>) -> bool {
    let primitive_flags = TypeFlags::String
        | TypeFlags::Number
        | TypeFlags::Boolean
        | TypeFlags::StringLiteral
        | TypeFlags::NumberLiteral
        | TypeFlags::BooleanLiteral
        | TypeFlags::Undefined
        | TypeFlags::Null;

    for member in tp.unroll_union_members(t).unwrap_or_default() {
        if member.flags().intersects(primitive_flags) {
            return true;
        }
    }
    false
}
