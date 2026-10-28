use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::EffectMajorVersion;

// go: internal/rules/generic_effect_services.go:14 GenericEffectServices 6d3c07b7c5ddee8a
// GenericEffectServices detects Effect Service class declarations that have type
// parameters (generics), which cannot be properly discriminated at runtime.
// This is a V3-only rule.
pub static GENERIC_EFFECT_SERVICES: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "genericEffectServices",
    group: "correctness",
    description: "Prevents services with type parameters that cannot be discriminated at runtime",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3"],
    codes: vec![tsdiag::EFFECT_SERVICES_WITH_TYPE_PARAMETERS_ARE_NOT_SUPPORTED_BECAUSE_THEY_CANNOT_BE_PROPERLY_DISCRIMINATED_AT_RUNTIME_WHICH_MAY_CAUSE_UNEXPECTED_BEHAVIOR_EFFECT_GENERIC_EFFECT_SERVICES.code()],
    replaced_by: vec![],
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
                let class_decl = node.as_class_declaration();
                if let Some(name) = node.name()
                    && class_decl.type_parameters().is_some()
                    && class_decl.heritage_clauses().is_some()
                    && let Some(class_sym) = ctx.type_parser.get_symbol_at_location(name)
                {
                    let class_type = ctx.checker().get_type_of_symbol_at_location(class_sym, Some(node));
                    if let Some(class_type) = class_type
                        && ctx.type_parser.is_context_tag(class_type)
                    {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ctx.get_error_range(name),
                            &tsdiag::EFFECT_SERVICES_WITH_TYPE_PARAMETERS_ARE_NOT_SUPPORTED_BECAUSE_THEY_CANNOT_BE_PROPERLY_DISCRIMINATED_AT_RUNTIME_WHICH_MAY_CAUSE_UNEXPECTED_BEHAVIOR_EFFECT_GENERIC_EFFECT_SERVICES,
                            &[],
                            &[],
                        ));
                        continue; // skip children
                    }
                }
            }

            // Enqueue children for further traversal
            node.for_each_child(&mut |child| {
                node_to_visit.push(child);
                false
            });
        }

        diags
    },
}
});
