use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};
use rustc_hash::{FxHashMap, FxHashSet};

// go: internal/rules/missing_effect_service_dependency.go:19 MissingEffectServiceDependency
// MissingEffectServiceDependency checks that Effect.Service dependencies satisfy
// all required layer inputs. It detects when a class extending Effect.Service has
// required services (from the layer's RIn) that are not provided in the dependencies
// configuration option. V3-only, default severity off.
pub static MISSING_EFFECT_SERVICE_DEPENDENCY: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "missingEffectServiceDependency",
    group: "style",
    description: "Checks that Effect.Service dependencies satisfy all required layer inputs",
    default_severity: Severity::Off,
    supported_effect: vec!["v3"],
    codes: vec![
        tsdiag::Service_0_is_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency.code(),
        tsdiag::Services_0_are_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency.code(),
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
                let d = check_service_dependencies(ctx, node);
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

// go: internal/rules/missing_effect_service_dependency.go:66 checkServiceDependencies
// checkServiceDependencies checks if a class extending Effect.Service has all
// required service dependencies satisfied.
pub(crate) fn check_service_dependencies(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Vec<P<Diagnostic>> {
    // Check if this class extends Effect.Service
    let Some(service_result) = ctx.type_parser.extends_effect_v3_service(node) else {
        return Vec::new();
    };

    let class_name = service_result.class_name();
    let options = service_result.options();

    // Get the class symbol and type
    let Some(class_sym) = ctx.type_parser.get_symbol_at_location(class_name) else {
        return Vec::new();
    };
    let Some(class_type) = ctx.checker().get_type_of_symbol_at_location(class_sym, Some(node))
    else {
        return Vec::new();
    };

    // Try DefaultWithoutDependencies first, then fall back to Default
    let mut default_prop =
        ctx.checker().get_property_of_type_exported(class_type, "DefaultWithoutDependencies");
    if default_prop.is_none() {
        default_prop = ctx.checker().get_property_of_type_exported(class_type, "Default");
    }
    let Some(default_prop) = default_prop else {
        return Vec::new();
    };

    let Some(default_type) = ctx.checker().get_type_of_symbol_at_location(default_prop, Some(node))
    else {
        return Vec::new();
    };

    // Parse as Layer type to get RIn
    let Some(layer) = ctx.type_parser.layer_type(default_type) else {
        return Vec::new();
    };

    // Use a shared memory map for both required and provided services
    let mut services_memory: FxHashMap<String, P<Type>> = FxHashMap::default();
    let mut exclude_never =
        |_: &mut TypeParser<'_>, t: P<Type>| -> bool { t.flags().intersects(TypeFlags::Never) };

    // Get all required service indexes from RIn
    let required_result = ctx.type_parser.append_to_unique_types_map(
        &mut services_memory,
        Some(layer.r_in()),
        Some(&mut exclude_never),
    );
    let required_indexes = required_result.all_indexes();

    if required_indexes.len() == 0 {
        return Vec::new();
    }

    // Process dependencies to find provided services
    let mut provided_indexes: FxHashSet<String> = FxHashSet::default();

    if let Some(options) = options
        && let Some(options_type) = ctx.type_parser.get_type_at_location(options)
        && let Some(dependencies_prop) = ctx.checker().get_property_of_type_exported(options_type, "dependencies")
        && let Some(dependencies_type) =
            ctx.checker().get_type_of_symbol_at_location(dependencies_prop, Some(options))
        // Get the number index type to extract individual dependency types
        && let Some(number_index_type) = ctx.checker().get_number_index_type(dependencies_type)
    {
        let dep_types = ctx.type_parser.unroll_union_members(number_index_type).unwrap_or_default();
        for dep_type in dep_types {
            // Parse each dependency as Layer type
            if let Some(dep_layer) = ctx.type_parser.layer_type(dep_type) {
                // Add the ROut of this dependency to provided services
                let provided_result = ctx.type_parser.append_to_unique_types_map(
                    &mut services_memory,
                    Some(dep_layer.r_out()),
                    Some(&mut exclude_never),
                );
                for (_, idx) in provided_result.all_indexes().all() {
                    provided_indexes.insert(idx.clone());
                }
            }
        }
    }

    // Find missing indexes: required but not provided
    let mut missing_indexes: Vec<&String> = Vec::new();
    for (_, idx) in required_indexes.all() {
        if !provided_indexes.contains(idx) {
            missing_indexes.push(idx);
        }
    }

    if missing_indexes.is_empty() {
        return Vec::new();
    }

    // Build the diagnostic
    let mut missing_type_names: Vec<String> = Vec::new();
    for idx in missing_indexes {
        if let Some(&t) = services_memory.get(idx) {
            missing_type_names.push(ctx.checker().type_to_string_exported(t));
        }
    }

    if missing_type_names.is_empty() {
        return Vec::new();
    }

    if missing_type_names.len() == 1 {
        // Singular
        let diag = ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(class_name),
            &tsdiag::Service_0_is_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency,
            &[],
            &[&missing_type_names[0]],
        );
        return vec![diag];
    }

    // Plural: format as 'X', 'Y'
    let mut quoted_names: Vec<String> = Vec::new();
    for name in &missing_type_names {
        quoted_names.push(format!("'{name}'"));
    }
    let formatted = quoted_names.join(", ");
    let diag = ctx.new_diagnostic(
        ctx.source_file,
        ctx.get_error_range(class_name),
        &tsdiag::Services_0_are_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency,
        &[],
        &[&formatted],
    );
    vec![diag]
}
