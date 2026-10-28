// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectMajorVersion;

use crate::global_json::{find_global_json_calls, global_json_recommendation};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THIS_CODE_USES_JSON_PARSE_OR_JSON_STRINGIFY_0_EFFECT_PREFER_SCHEMA_OVER_JSON;

// go: internal/rules/prefer_schema_over_json.go:15 PreferSchemaOverJson ba1bee833adc6dbd
// PreferSchemaOverJson is deprecated: it reports exactly what GlobalJsonInEffect
// reports, with its original message, so configurations that enable it keep
// their coverage.
pub static PREFER_SCHEMA_OVER_JSON: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSchemaOverJson",
    group: "effectNative",
    description: "Deprecated: use globalJsonInEffect and globalJson instead. Suggests using Effect Schema for JSON operations instead of JSON.parse/JSON.stringify inside Effect code",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec!["globalJsonInEffect", "globalJson"],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let recommendation = global_json_recommendation(
            ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4,
        );
        let mut diags = Vec::new();
        for call in find_global_json_calls(ctx, true) {
            diags.push(ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(call.node),
                MESSAGE,
                &[],
                &[recommendation],
            ));
        }
        diags
    },
});
