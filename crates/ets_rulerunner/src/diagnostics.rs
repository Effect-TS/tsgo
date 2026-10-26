use std::any::Any;

use ets_directives::{Directive, DirectiveSet};
use ets_etscore::{EffectPluginOptions, ResolvedEffectPluginOptions, Severity};
use ets_rule::Rule;
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::checker::{Checker, Program};
use ets_shim::core::TextRange;
use ets_shim::core::context::Context;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;
use rustc_hash::FxHashMap;

// go: internal/rulerunner/diagnostics.go:23 RuleDiagnostic
// RuleDiagnostic pairs a diagnostic with its rule for directive processing.
pub struct RuleDiagnostic {
    pub rule_name: &'static str,
    pub rule: &'static Rule,
    pub diagnostic: P<Diagnostic>,
}

// go: internal/rulerunner/diagnostics.go:33 MinVisibleSeverity
// MinVisibleSeverity returns the least visible severity that can surface in
// the current execution mode: in CLI (tsc) mode without includeSuggestionsInTsc
// only warnings and errors are printed, so SeverityWarning; everywhere else
// (LSP, tests) every severity surfaces, so SeverityMessage.
pub fn min_visible_severity(effect_config: Option<&EffectPluginOptions>) -> Severity {
    if ets_etscore::is_command_line_mode()
        && !EffectPluginOptions::get_include_suggestions_in_tsc(effect_config)
    {
        return Severity::Warning;
    }
    Severity::Message
}

/// Rust-only: the concrete compiler program behind a `checker.Program`.
///
/// Go's `typeparser.NewTypeParser(program, c)` takes the `checker.Program` and type-asserts the capabilities it
/// needs; `ets_typeparser::TypeParser` holds the concrete `compiler.Program` (`docs/TSRS_API.md`, "Gaps"). The
/// checker hooks and the other callers run on compiler programs (the after-check callback fires from a compiler
/// program's checkers), so the downcast (tsrs patch 002: `Program: Any`) always succeeds.
pub fn compiler_program(program: &'static dyn Program) -> &'static ets_shim::compiler::Program {
    let program: &'static dyn Any = program;
    program
        .downcast_ref::<ets_shim::compiler::Program>()
        .expect("the Effect rules run on a compiler program")
}

// go: internal/rulerunner/diagnostics.go:44 Run
// Run executes Effect diagnostics for a source file and returns diagnostics without emitting them.
// minSeverity is the least visible severity the caller can surface (see
// MinVisibleSeverity); rules whose configured severity is below it are skipped
// pre-execution unless a directive in the file references them.
//
// Rust: Go's nil and empty `ruleNames` differ (all rules, no rule): `None` and `Some(&[])`. The error is Go's
// error text.
pub fn run(
    ctx: &Context,
    program: &'static dyn Program,
    c: &mut Checker,
    sf: P<SourceFile>,
    effect_config: Option<&EffectPluginOptions>,
    rule_names: Option<&[&str]>,
    min_severity: Severity,
) -> Result<Vec<P<Diagnostic>>, String> {
    if sf.is_declaration_file() || program.is_source_file_from_external_library(sf) {
        return Ok(Vec::new());
    }

    let Some(effect_config) = effect_config else {
        return Ok(Vec::new());
    };

    let effective_config = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
        Some(effect_config),
        sf.file_name(),
        &program.options().config_file_path,
        program.use_case_sensitive_file_names(),
    );

    let resolved_severity = ets_pluginoptions::resolve_diagnostic_severity_for_file(
        Some(effect_config),
        sf.file_name(),
        &program.options().config_file_path,
        program.use_case_sensitive_file_names(),
    );

    if !ets_etscore::diagnostics_enabled(Some(effect_config)) || resolved_severity.is_none() {
        return Ok(Vec::new());
    }
    let resolved_severity = resolved_severity.as_deref();

    let selected_rules = select_rules(rule_names)?;
    if selected_rules.is_empty() {
        return Ok(Vec::new());
    }

    let mut tp = ets_typeparser::new_type_parser(compiler_program(program), c);
    let source_text = sf.text();
    let effect_directives = ets_directives::collect_effect_directives(source_text);
    let mut directive_set = ets_directives::build_directive_set(&effect_directives);

    if directive_set.is_skip_file("*") {
        return Ok(Vec::new());
    }

    let all_diagnostics = collect_diagnostics(
        ctx,
        program,
        &mut tp,
        sf,
        effect_config,
        effective_config.as_ref(),
        resolved_severity,
        &directive_set,
        &selected_rules,
        min_severity,
    );
    let mut final_diagnostics = transform_diagnostics(
        &all_diagnostics,
        sf,
        &mut directive_set,
        resolved_severity,
        min_severity,
    );
    final_diagnostics.extend(unused_directive_diagnostics(
        sf,
        &effect_directives,
        &directive_set,
        resolved_severity,
    ));

    Ok(final_diagnostics)
}

// go: internal/rulerunner/diagnostics.go:95 selectRules
pub(crate) fn select_rules(rule_names: Option<&[&str]>) -> Result<Vec<&'static Rule>, String> {
    if rule_names.is_some_and(<[&str]>::is_empty) {
        return Ok(Vec::new());
    }

    let Some(rule_names) = rule_names else {
        let mut selected = Vec::with_capacity(ets_rules::ALL.len());
        for r in ets_rules::ALL.iter() {
            selected.push(r);
        }
        return Ok(selected);
    };

    let mut selected = Vec::with_capacity(rule_names.len());
    for &name in rule_names {
        let Some(idx) = ets_rules::ALL.iter().position(|r| r.name == name) else {
            return Err(format!("unknown Effect diagnostic rule {}", ets_go::strconv::quote(name)));
        };
        selected.push(&ets_rules::ALL[idx]);
    }
    Ok(selected)
}

// go: internal/rulerunner/diagnostics.go:120 collectDiagnostics
// collectDiagnostics runs all enabled rules and collects their diagnostics.
//
// Rust: the checker comes with the type parser (`ets_rule::Context`).
pub(crate) fn collect_diagnostics(
    ctx: &Context,
    program: &'static dyn Program,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    global_config: &EffectPluginOptions,
    options: Option<&ResolvedEffectPluginOptions>,
    resolved_severity: Option<&FxHashMap<String, Severity>>,
    directive_set: &DirectiveSet,
    selected_rules: &[&'static Rule],
    min_severity: Severity,
) -> Vec<RuleDiagnostic> {
    let mut results = Vec::new();

    for &r in selected_rules {
        let (mut config_severity, configured_explicitly) =
            severity_from_map(resolved_severity, r.name);
        if !configured_explicitly {
            config_severity = r.default_severity;
        }
        if !global_config.skip_disabled_optimization
            && config_severity.is_off()
            && !directive_set.has_enabling_directive(r.name)
        {
            continue;
        }
        // A rule below the minimum visible severity can only surface if a
        // directive raises it, and it must still run when any directive
        // references it so the directive is marked used for unusedDirective
        // tracking.
        if !global_config.skip_disabled_optimization
            && !config_severity.at_least_as_visible_as(min_severity)
            && !directive_set.has_any_directive_for_rule(r.name)
        {
            continue;
        }

        if directive_set.is_skip_file(r.name) {
            continue;
        }

        let mut rule_ctx = ets_rule::new_context(ctx, program, tp, sf, options, r.default_severity);
        let diags = (r.run)(&mut rule_ctx);

        for diag in diags {
            results.push(RuleDiagnostic { rule_name: r.name, rule: r, diagnostic: diag });
        }
    }

    results
}

// go: internal/rulerunner/diagnostics.go:172 transformDiagnostics
pub(crate) fn transform_diagnostics(
    diags: &[RuleDiagnostic],
    sf: P<SourceFile>,
    directive_set: &mut DirectiveSet,
    resolved_severity: Option<&FxHashMap<String, Severity>>,
    min_severity: Severity,
) -> Vec<P<Diagnostic>> {
    let mut results = Vec::new();
    let line_map = sf.ecma_line_map();

    for rd in diags {
        // Rust: a rule diagnostic always has a location (Go's `Pos()` of a located diagnostic).
        let line = ets_shim::scanner::compute_line_of_position(
            line_map,
            rd.diagnostic.pos().unwrap_or_default(),
        ) as i64;

        let (mut default_severity, configured_explicitly) =
            severity_from_map(resolved_severity, rd.rule_name);
        if !configured_explicitly {
            default_severity = rd.rule.default_severity;
        }

        let effective_severity = directive_set.get_effective_severity_and_mark_used(
            rd.rule_name,
            line,
            default_severity,
        );

        if effective_severity.is_off() {
            continue;
        }
        if !effective_severity.at_least_as_visible_as(min_severity) {
            continue;
        }

        let original_category = rd.diagnostic.category();
        let new_category = ets_directives::to_category(effective_severity);

        if original_category != new_category {
            results.push(create_transformed_diagnostic(rd.diagnostic, new_category));
        } else {
            results.push(rd.diagnostic);
        }
    }

    results
}

// go: internal/rulerunner/diagnostics.go:216 createTransformedDiagnostic
pub(crate) fn create_transformed_diagnostic(
    original: P<Diagnostic>,
    new_category: tsdiag::Category,
) -> P<Diagnostic> {
    ets_shim::ast::new_diagnostic_from_serialized(
        original.file(),
        original.loc().unwrap_or(TextRange::synthetic()),
        original.code(),
        new_category,
        original.message_key(),
        original.message_args().to_vec(),
        original.message_chain(),
        original.related_information(),
        original.reports_unnecessary(),
        original.reports_deprecated(),
        original.skipped_on_no_emit(),
    )
}

// go: internal/rulerunner/diagnostics.go:232 unusedDirectiveDiagnostics
pub(crate) fn unused_directive_diagnostics(
    sf: P<SourceFile>,
    all_directives: &[Directive],
    directive_set: &DirectiveSet,
    resolved_severity: Option<&FxHashMap<String, Severity>>,
) -> Vec<P<Diagnostic>> {
    let (mut severity, ok) = severity_from_map(resolved_severity, ets_rule::UNUSED_DIRECTIVE_NAME);
    if !ok {
        severity = Severity::Warning;
    }
    if severity.is_off() {
        return Vec::new();
    }

    let unused = directive_set.get_unused_next_line_directives(all_directives);
    if unused.is_empty() {
        return Vec::new();
    }

    let mut diags = Vec::with_capacity(unused.len());
    for d in &unused {
        diags.push(ets_shim::ast::new_diagnostic_from_serialized(
            Some(sf),
            TextRange::new(d.pos as u32, d.end as u32),
            tsdiag::X_EFFECT_DIAGNOSTICS_DIRECTIVE_HAS_NO_EFFECT.code(),
            ets_directives::to_category(severity),
            tsdiag::X_EFFECT_DIAGNOSTICS_DIRECTIVE_HAS_NO_EFFECT.key(),
            Vec::new(),
            &[],
            &[],
            false,
            false,
            false,
        ));
    }
    diags
}

// go: internal/rulerunner/diagnostics.go:265 severityFromMap
pub(crate) fn severity_from_map(
    resolved: Option<&FxHashMap<String, Severity>>,
    rule_name: &str,
) -> (Severity, bool) {
    let Some(resolved) = resolved else {
        return (Severity::Error, false);
    };
    match resolved.get(rule_name) {
        Some(&severity) => (severity, true),
        None => (Severity::default(), false),
    }
}
