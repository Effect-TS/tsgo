use std::sync::{Arc, LazyLock};

use ets_etscore::{
    EFFECT_FN_INFERRED_SPAN, EFFECT_FN_NO_SPAN, EFFECT_FN_SPAN, EFFECT_FN_SUGGESTED_SPAN,
    EFFECT_FN_UNTRACED, ResolvedEffectPluginOptions, Severity,
};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectFnOpportunityResult, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_CAN_BE_REWRITTEN_IN_THE_REUSABLE_FUNCTION_FORM_0_EFFECT_EFFECT_FN_OPPORTUNITY;

// go: internal/rules/effect_fn_opportunity.go:17 EffectFnOpportunity
// EffectFnOpportunity detects functions that can be rewritten as Effect.fn calls.
pub static EFFECT_FN_OPPORTUNITY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectFnOpportunity",
    group: "style",
    description: "Suggests using Effect.fn for functions that return an Effect",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let effect_config = ctx.options;
        let source_file = ctx.source_file;
        let matches = analyze_effect_fn_opportunity(ctx.type_parser, source_file);
        let mut diags = Vec::new();
        for m in &matches {
            let fix_name = first_available_fix_name(&m.result, effect_config);
            if fix_name.is_empty() {
                continue;
            }
            let expected_signature = build_expected_signature(ctx.source_file, m, fix_name);
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&expected_signature],
            ));
        }
        diags
    },
});

// go: internal/rules/effect_fn_opportunity.go:43 EffectFnOpportunityMatch
// EffectFnOpportunityMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the effectFnOpportunity pattern.
#[derive(Clone)]
pub struct EffectFnOpportunityMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub result: Arc<EffectFnOpportunityResult>,
}

// go: internal/rules/effect_fn_opportunity.go:51 AnalyzeEffectFnOpportunity
// AnalyzeEffectFnOpportunity finds all functions that can be converted to Effect.fn
// in the given source file.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_fn_opportunity(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectFnOpportunityMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<EffectFnOpportunityMatch>,
    ) -> bool {
        if let Some(result) = tp.parse_effect_fn_opportunity(n) {
            // Report on the name identifier if available, otherwise the function node
            // Rust: the parser's name identifier is never nil.
            let report_node = result.name_identifier();
            matches.push(EffectFnOpportunityMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, report_node),
                result,
            });
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);

    matches
}

// go: internal/rules/effect_fn_opportunity.go:85 firstAvailableFixName
// firstAvailableFixName determines which fix variant would be first (highest priority)
// for the given match, matching the reference implementation's fix ordering.
// Returns empty string when no fix variant passes the config filter.
pub(crate) fn first_available_fix_name(
    result: &EffectFnOpportunityResult,
    effect_config: Option<&ResolvedEffectPluginOptions>,
) -> &'static str {
    let includes =
        |variant: &str| ResolvedEffectPluginOptions::effect_fn_includes(effect_config, variant);
    // Priority order matches upstream: withSpan > untraced > noSpan > spanInferred > spanSuggested
    if includes(EFFECT_FN_SPAN) && result.explicit_trace_expression().is_some() {
        return "effectFnOpportunity_toEffectFnWithSpan";
    }
    if includes(EFFECT_FN_UNTRACED) && result.generator_function().is_some() {
        return "effectFnOpportunity_toEffectFnUntraced";
    }
    if includes(EFFECT_FN_NO_SPAN) {
        return "effectFnOpportunity_toEffectFnNoSpan";
    }
    if result.explicit_trace_expression().is_none() {
        if includes(EFFECT_FN_INFERRED_SPAN) && !result.inferred_trace_name().is_empty() {
            return "effectFnOpportunity_toEffectFnSpanInferred";
        }
        if includes(EFFECT_FN_SUGGESTED_SPAN)
            && !result.suggested_trace_name().is_empty()
            && (!includes(EFFECT_FN_INFERRED_SPAN)
                || result.suggested_trace_name() != result.inferred_trace_name())
        {
            return "effectFnOpportunity_toEffectFnSpanSuggested";
        }
    }
    ""
}

// go: internal/rules/effect_fn_opportunity.go:111 buildExpectedSignature
// buildExpectedSignature constructs the human-readable expected signature string
// for the diagnostic message. The format matches the reference implementation:
// it shows how the function would look after conversion with the highest-priority fix.
pub(crate) fn build_expected_signature(
    sf: P<SourceFile>,
    m: &EffectFnOpportunityMatch,
    fix_name: &str,
) -> String {
    let result = &m.result;

    // Get the Effect module name
    let mut effect_module_name = "Effect".to_string();
    if let Some(effect_module) = result.effect_module()
        && effect_module.kind() == Kind::Identifier
    {
        effect_module_name = ets_shim::scanner::get_text_of_node(effect_module);
    }

    // Build type parameter string: <T, U, ...>
    let type_param_str = get_type_param_string(result.target_node());

    // Build parameter names string: (x, y, ...)
    let param_str = get_param_names_string(result.target_node());

    let fn_signature = if result.has_gen_body() {
        "function*".to_string() + &type_param_str + "(" + &param_str + ") { ... }"
    } else if result.target_node().kind() == Kind::ArrowFunction {
        type_param_str + "(" + &param_str + ") => { ... }"
    } else {
        "function".to_string() + &type_param_str + "(" + &param_str + ") { ... }"
    };

    let pipe_args = result.pipe_arguments();
    let mut pipe_args_for_with_span = pipe_args;
    if pipe_args.len() > 0 {
        pipe_args_for_with_span = pipe_args.slice(0, pipe_args.len() - 1);
    }

    let pipe_args_suffix = |count: i64| -> &'static str {
        if count > 0 {
            return ", ...pipeTransformations";
        }
        ""
    };

    match fix_name {
        "effectFnOpportunity_toEffectFnWithSpan" => {
            let mut trace_name = String::new();
            if let Some(trace) = result.explicit_trace_expression() {
                let pos = trace.pos().expect_source("parsed node") as usize;
                let end = trace.end().expect_source("parsed node") as usize;
                trace_name = ets_go::strings::trim_space(&sf.text()[pos..end]).to_string();
            }
            effect_module_name
                + ".fn("
                + &trace_name
                + ")("
                + &fn_signature
                + pipe_args_suffix(pipe_args_for_with_span.len())
                + ")"
        }

        "effectFnOpportunity_toEffectFnUntraced" => {
            effect_module_name
                + ".fnUntraced("
                + &fn_signature
                + pipe_args_suffix(pipe_args.len())
                + ")"
        }

        "effectFnOpportunity_toEffectFnNoSpan" => {
            effect_module_name + ".fn(" + &fn_signature + pipe_args_suffix(pipe_args.len()) + ")"
        }

        "effectFnOpportunity_toEffectFnSpanInferred" => {
            effect_module_name
                + ".fn(\""
                + result.inferred_trace_name()
                + "\")("
                + &fn_signature
                + pipe_args_suffix(pipe_args.len())
                + ")"
        }

        "effectFnOpportunity_toEffectFnSpanSuggested" => {
            effect_module_name
                + ".fn(\""
                + result.suggested_trace_name()
                + "\")("
                + &fn_signature
                + pipe_args_suffix(pipe_args.len())
                + ")"
        }

        _ => effect_module_name + ".fn(" + &fn_signature + ")",
    }
}

// go: internal/rules/effect_fn_opportunity.go:176 getTypeParamString
// getTypeParamString extracts type parameter names from a function node and returns
// a string like "<T, U>" or "" if there are no type parameters.
pub(crate) fn get_type_param_string(fn_node: P<Node>) -> String {
    let type_params = ets_typeparser::get_function_like_type_parameters(fn_node);

    let Some(type_params) = type_params else {
        return String::new();
    };
    if type_params.nodes().is_empty() {
        return String::new();
    }

    let mut names: Vec<String> = Vec::new();
    for &tp in type_params.nodes() {
        if !ast::is_type_parameter_declaration(tp) {
            continue;
        }
        if let Some(name) = tp.name() {
            names.push(ets_shim::scanner::get_text_of_node(name));
        }
    }
    if names.is_empty() {
        return String::new();
    }
    "<".to_string() + &names.join(", ") + ">"
}

// go: internal/rules/effect_fn_opportunity.go:202 getParamNamesString
// getParamNamesString extracts parameter names from a function node and returns
// a string like "x, y" or "" if there are no parameters.
// For destructuring patterns, uses "_" as a placeholder.
pub(crate) fn get_param_names_string(fn_node: P<Node>) -> String {
    let params = ets_typeparser::get_function_like_parameters(fn_node);

    let Some(params) = params else {
        return String::new();
    };
    if params.nodes().is_empty() {
        return String::new();
    }

    let mut names: Vec<String> = Vec::new();
    for &p in params.nodes() {
        match p.name() {
            Some(name) if name.kind() == Kind::Identifier => {
                names.push(ets_shim::scanner::get_text_of_node(name));
            }
            _ => names.push("_".to_string()),
        }
    }
    names.join(", ")
}
