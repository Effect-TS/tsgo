// Package effectconfigcheck validates resolved Effect compiler options.

use std::sync::Arc;

use ets_etscore::Severity;
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::core::CompilerOptions;
use ets_shim::diagnostics;
use rustc_hash::FxHashMap;

// go: internal/effectconfigcheck/validation.go:17 Register d3af5212dd47568b
pub fn register() {
    ets_shim::tsoptions::register_validate_compiler_options_callback(Arc::new(validate));
}

// go: internal/effectconfigcheck/validation.go:21 validate 430887de3f635c6d
pub(crate) fn validate(
    options: Option<&CompilerOptions>,
    source_file: Option<P<SourceFile>>,
) -> Vec<P<Diagnostic>> {
    let Some(options) = options else {
        return Vec::new();
    };
    let effect = options.effect.get();
    if !ets_etscore::diagnostics_enabled(effect.as_deref()) {
        return Vec::new();
    }
    let Some(config) = effect.as_deref() else {
        return Vec::new();
    };
    let severity = config
        .diagnostic_severity
        .as_ref()
        .and_then(|severities| severities.get(ets_rule::UNKNOWN_RULE_NAME_NAME))
        .copied()
        .unwrap_or(Severity::Warning);
    if severity.is_off() {
        return Vec::new();
    }

    // Syntax is used only to locate diagnostics; validation uses the merged options.
    let plugin = effect_plugin_syntax(source_file);
    let mut result = Vec::new();
    let mut check = |severities: Option<&FxHashMap<String, Severity>>, syntax: Option<P<Node>>| {
        let mut unknown: Vec<&str> = Vec::new();
        for name in severities.into_iter().flat_map(FxHashMap::keys) {
            if name != ets_rule::UNUSED_DIRECTIVE_NAME
                && name != ets_rule::UNKNOWN_RULE_NAME_NAME
                && ets_rule::by_name(&ets_rules::ALL, name).is_none()
            {
                unknown.push(name);
            }
        }
        // Maps have no iteration order; keep CLI output and baselines deterministic.
        unknown.sort_unstable();
        for name in unknown {
            let node = find_property(syntax, name).and_then(|property| property.name());
            let diagnostic =
                ets_shim::tsoptions::create_diagnostic_for_node_in_source_file_or_compiler_diagnostic(
                    source_file,
                    node,
                    &diagnostics::UNKNOWN_EFFECT_DIAGNOSTIC_RULE_0_IN_DIAGNOSTIC_SEVERITY_EFFECT_UNKNOWN_RULE_NAME,
                    &[&name],
                );
            diagnostic.set_category(ets_directives::to_category(severity));
            result.push(diagnostic);
        }
    };
    check(config.diagnostic_severity.as_ref(), property_value(plugin, "diagnosticSeverity"));

    // Inherited overrides precede local overrides. Only local object entries have
    // syntax in this file, and the parser skips non-object entries.
    let mut local_overrides: Vec<P<Node>> = Vec::new();
    for &node in array_elements(property_value(plugin, "overrides")) {
        if ast::is_object_literal_expression(node) {
            local_overrides.push(node);
        }
    }
    let overrides = config.overrides.as_deref().unwrap_or_default();
    let local_start = overrides.len() as i64 - local_overrides.len() as i64;
    for (i, override_) in overrides.iter().enumerate() {
        let mut syntax = None;
        if local_start >= 0 && i as i64 >= local_start {
            syntax = property_value(
                property_value(Some(local_overrides[i - local_start as usize]), "options"),
                "diagnosticSeverity",
            );
        }
        check(override_.options.diagnostic_severity.as_ref(), syntax);
    }
    result
}

// go: internal/effectconfigcheck/validation.go:81 effectPluginSyntax 229c909650d06bb7
pub(crate) fn effect_plugin_syntax(source_file: Option<P<SourceFile>>) -> Option<P<Node>> {
    let compiler_options =
        ets_shim::tsoptions::for_each_ts_config_prop_array(source_file, "compilerOptions", Some)?;
    for &plugin in array_elements(property_value(compiler_options.initializer(), "plugins")) {
        let name = property_value(Some(plugin), "name");
        if let Some(name) = name
            && ast::is_string_literal_like(name)
            && name.text() == ets_etscore::EFFECT_PLUGIN_NAME
        {
            return Some(plugin);
        }
    }
    None
}

// go: internal/effectconfigcheck/validation.go:97 findProperty bcdac960da91b16c
// Rust: the property assignment node (Go's `*ast.PropertyAssignment`).
pub(crate) fn find_property(node: Option<P<Node>>, name: &str) -> Option<P<Node>> {
    let node = node?;
    if !ast::is_object_literal_expression(node) {
        return None;
    }
    let mut result = None;
    for &property in node.properties() {
        if ast::is_property_assignment(property)
            && property.name().is_some_and(|n| ast::get_text_of_property_name(n) == name)
        {
            // JSON parsing retains the last value for duplicate keys.
            result = Some(property);
        }
    }
    result
}

// go: internal/effectconfigcheck/validation.go:111 propertyValue 8ee149fc306c6596
pub(crate) fn property_value(node: Option<P<Node>>, name: &str) -> Option<P<Node>> {
    if let Some(property) = find_property(node, name) {
        return property.initializer();
    }
    None
}

// go: internal/effectconfigcheck/validation.go:118 arrayElements 1b02ec45521e85d3
pub(crate) fn array_elements(node: Option<P<Node>>) -> &'static [P<Node>] {
    if let Some(node) = node
        && ast::is_array_literal_expression(node)
    {
        return node.elements();
    }
    &[]
}
