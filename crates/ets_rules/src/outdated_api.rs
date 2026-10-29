use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};

use crate::*;

// go: internal/rules/outdated_api.go:17 OutdatedApi d9e51ae4c857c8a0
// OutdatedApi detects usage of Effect v3 APIs in projects targeting Effect v4.
pub static OUTDATED_API: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "outdatedApi",
        group: "correctness",
        description: "Detects usage of APIs that have been removed or renamed in Effect v4",
        default_severity: Severity::Warning,
        supported_effect: vec!["v4"],
        codes: vec![
            tsdiag::THIS_PROJECT_TARGETS_EFFECT_V4_BUT_THIS_CODE_USES_THE_EFFECT_V3_API_0_THE_REFERENCED_API_BELONGS_TO_THE_V3_SURFACE_RATHER_THAN_THE_CONFIGURED_V4_SURFACE_1_EFFECT_OUTDATED_API.code(),
            tsdiag::THIS_PROJECT_TARGETS_EFFECT_V4_BUT_THIS_CODE_USES_EFFECT_V3_AP_IS_THE_REFERENCED_API_BELONGS_TO_THE_V3_SURFACE_RATHER_THAN_THE_CONFIGURED_V4_SURFACE_EFFECT_OUTDATED_API.code(),
        ],
        replaced_by: vec![],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_outdated_api(ctx.type_parser, ctx.source_file);
            if matches.is_empty() {
                return Vec::new();
            }

            let mut diags = Vec::with_capacity(matches.len() + 1);
            for m in &matches {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    &tsdiag::THIS_PROJECT_TARGETS_EFFECT_V4_BUT_THIS_CODE_USES_THE_EFFECT_V3_API_0_THE_REFERENCED_API_BELONGS_TO_THE_V3_SURFACE_RATHER_THAN_THE_CONFIGURED_V4_SURFACE_1_EFFECT_OUTDATED_API,
                    &[],
                    &[&m.property_name, &m.migration_hint],
                ));
            }

            // Append global summary diagnostic at position 0:0
            diags.push(ctx.new_diagnostic(
                ctx.source_file,
                TextRange::new(0, 0),
                &tsdiag::THIS_PROJECT_TARGETS_EFFECT_V4_BUT_THIS_CODE_USES_EFFECT_V3_AP_IS_THE_REFERENCED_API_BELONGS_TO_THE_V3_SURFACE_RATHER_THAN_THE_CONFIGURED_V4_SURFACE_EFFECT_OUTDATED_API,
                &[],
                &[],
            ));

            diags
        },
    }
});

// go: internal/rules/outdated_api.go:58 OutdatedApiMatch aa554d851c743a84
// OutdatedApiMatch holds the analysis results for one outdated API usage.
#[derive(Clone)]
pub struct OutdatedApiMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub property_name: String,
    pub migration_hint: String,
    pub migration: Migration,
}

// go: internal/rules/outdated_api.go:67 AnalyzeOutdatedApi b7cc980964c98292
// AnalyzeOutdatedApi finds all usages of Effect v3 APIs in an Effect v4 project.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped.
pub fn analyze_outdated_api(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> Vec<OutdatedApiMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<OutdatedApiMatch>,
    ) -> bool {
        if n.kind() == Kind::PropertyAccessExpression
            && let Some(m) = check_outdated_api_property_access(tp, sf, n)
        {
            matches.push(m);
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);

    matches
}

// go: internal/rules/outdated_api.go:95 checkOutdatedApiPropertyAccess cad5bdfb83598493
//
// Rust: the checker is the parser's.
pub(crate) fn check_outdated_api_property_access(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    n: P<Node>,
) -> Option<OutdatedApiMatch> {
    let prop = n.as_property_access_expression();

    let name_node = prop.name;

    let identifier_name = name_node.text();
    if identifier_name.is_empty() {
        return None;
    }

    let migration = EFFECT_MODULE_MIGRATION_DB.get(identifier_name)?;

    // Skip unchanged entries — they exist in both v3 and v4
    if migration.tag == MigrationTag::Unchanged {
        return None;
    }

    // Get the type of the expression (left side of the property access)
    // Verify the expression (left side of the property access) references the
    // Effect module, before computing its type.
    let expr = prop.expression;
    if !tp.is_expression_effect_module(expr) {
        return None;
    }

    let target_type = tp.get_type_at_location(expr)?;

    // Only report if the property does NOT exist on the target type
    // (confirming it's a v3-only API)
    if tp.checker().get_property_of_type(target_type, identifier_name).is_some() {
        return None;
    }

    Some(OutdatedApiMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, name_node),
        property_name: identifier_name.to_string(),
        migration_hint: migration_hint_text(migration),
        migration: migration.clone(),
    })
}

// go: internal/rules/outdated_api.go:148 migrationHintText 9688d29085592404
pub(crate) fn migration_hint_text(m: &Migration) -> String {
    match m.tag {
        MigrationTag::Removed => m.alternative_pattern.to_string(),
        MigrationTag::RenamedSameBehaviour => format!("Renamed to {}.", m.new_name),
        MigrationTag::RenamedAndNeedsOptions => {
            format!("Renamed to {}. {}", m.new_name, m.options_instructions)
        }
        MigrationTag::Unchanged => String::new(),
    }
}
