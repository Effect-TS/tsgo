use ets_etscore::{
    AnyValue, EFFECT_PLUGIN_NAME, EffectPluginOptions, ResolvedEffectPluginOptions, Severity,
};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::checker::Checker;
use ets_shim::compiler::Program;
use ets_shim::core::TextRange;
use ets_shim::core::context::Context;
use ets_shim::lsproto;
use rustc_hash::FxHashMap;

// go: etsoxlintrunner/runner.go:25 init 18d0e99d1400ca67
pub(crate) fn init() {
    // Oxlint loads this runner without the automatic checker diagnostics hooks.
    ets_effectconfigraw::register();
}

// go: etsoxlintrunner/runner.go:31 ReportedDiagnostic 3792534ff6bcf603
// ReportedDiagnostic is a runner-neutral diagnostic ready for an external integration.
//
// Rust: Go's `Suggestions` closure captures the checker; it is an `FnOnce` borrowing it for the report call (`'s`).
pub struct ReportedDiagnostic<'s> {
    pub range: TextRange,
    pub message_id: String,
    pub description: String,
    pub related_information: Vec<ReportedRelatedInformation>,
    pub suggestions: Box<dyn FnOnce() -> Vec<ReportedSuggestion> + 's>,
}

// go: etsoxlintrunner/runner.go:40 ReportedSuggestion 5e6d6f884ebc6887
// ReportedSuggestion is an independently selectable correction for a diagnostic.
pub struct ReportedSuggestion {
    pub description: String,
    pub edits: Vec<ReportedEdit>,
}

// go: etsoxlintrunner/runner.go:46 ReportedEdit bb9bacff5105cbbd
// ReportedEdit replaces Range with Text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedEdit {
    pub range: TextRange,
    pub text: String,
}

// go: etsoxlintrunner/runner.go:52 ReportedRelatedInformation 54db6ddc29cbac55
// ReportedRelatedInformation is a location and message related to a reported diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedRelatedInformation {
    pub file_name: String,
    pub range: TextRange,
    pub description: String,
}

/// Rust-only: Go's `DiagnosticAdapter.Report` callback, called once per diagnostic.
pub type ReportFn<'a> = dyn for<'s> FnMut(ReportedDiagnostic<'s>) + 'a;

// go: etsoxlintrunner/runner.go:59 DiagnosticAdapter c12a8fd817c175b8
// DiagnosticAdapter provides integration-specific message localization and reporting.
//
// Rust: Go's nil callbacks are `None`.
#[derive(Default)]
pub struct DiagnosticAdapter<'a> {
    pub message: Option<&'a dyn Fn(P<Diagnostic>) -> String>,
    pub report: Option<&'a mut ReportFn<'a>>,
}

// go: etsoxlintrunner/runner.go:65 RunRule 1beb6b164dc65cfb
// RunRule executes one Effect rule with the supplied program checker.
pub fn run_rule(
    ctx: &Context,
    program: &'static Program,
    c: &mut Checker,
    sf: P<SourceFile>,
    options: Option<&EffectPluginOptions>,
    rule_name: &str,
) -> Result<Vec<P<Diagnostic>>, String> {
    let normalized = normalize_options(options, rule_name);

    ets_rulerunner::run(
        ctx,
        program,
        c,
        sf,
        Some(&normalized),
        Some(&[rule_name]),
        ets_rulerunner::min_visible_severity(Some(&normalized)),
    )
}

/// Rust-only: Go's `[]any{map[string]any{"name": etscore.EffectPluginName}}` literal, as `ParseFromPlugins` reads it.
enum PluginValue {
    String(&'static str),
    Slice(Vec<PluginValue>),
    Map(Vec<(&'static str, PluginValue)>),
}

impl AnyValue for PluginValue {
    fn is_nil(&self) -> bool {
        false
    }
    fn as_bool(&self) -> Option<bool> {
        None
    }
    fn as_float64(&self) -> Option<f64> {
        None
    }
    fn as_string(&self) -> Option<&str> {
        match self {
            PluginValue::String(s) => Some(s),
            _ => None,
        }
    }
    fn as_slice(&self) -> Option<&[Self]> {
        match self {
            PluginValue::Slice(s) => Some(s),
            _ => None,
        }
    }
    fn as_string_any_map(&self) -> Option<Vec<(&str, &Self)>> {
        match self {
            PluginValue::Map(m) => Some(m.iter().map(|(k, v)| (*k, v)).collect()),
            _ => None,
        }
    }
}

// go: etsoxlintrunner/runner.go:78 normalizeOptions 3435d361d1df8e7d
//
// Rust: Go dereferences the parsed options, which are never nil for the plugin name alone.
pub(crate) fn normalize_options(
    options: Option<&EffectPluginOptions>,
    rule_name: &str,
) -> EffectPluginOptions {
    let parsed;
    let options = if let Some(options) = options {
        options
    } else {
        parsed = ets_etscore::parse_from_plugins(&PluginValue::Slice(vec![PluginValue::Map(
            vec![("name", PluginValue::String(EFFECT_PLUGIN_NAME))],
        )]));
        parsed.as_ref().expect("the plugin name alone parses")
    };

    let mut normalized = options.clone();
    normalized.diagnostics = true;
    normalized.diagnostic_severity = Some(FxHashMap::from_iter([
        (rule_name.to_string(), Severity::Error),
        ("unusedDirective".to_string(), Severity::Off),
    ]));
    normalized.overrides = options.overrides.as_ref().filter(|o| !o.is_empty()).cloned();
    for override_ in normalized.overrides.iter_mut().flatten() {
        override_.options.diagnostic_severity = None;
    }
    normalized
}

// go: etsoxlintrunner/runner.go:97 RunRuleAndReport 632aabbe49b22003
// RunRuleAndReport executes one Effect rule and reports converted diagnostics through adapter.
//
// Rust: Go's program, checker and source file may be nil until the adapter is checked (its test passes nil); they
// are dereferenced after.
pub fn run_rule_and_report(
    ctx: &Context,
    program: Option<&'static Program>,
    c: Option<&mut Checker>,
    sf: Option<P<SourceFile>>,
    options: Option<&EffectPluginOptions>,
    rule_name: &str,
    adapter: DiagnosticAdapter<'_>,
) -> Result<(), String> {
    if adapter.message.is_none() {
        return Err("diagnostic adapter Message callback is required".to_string());
    }
    if adapter.report.is_none() {
        return Err("diagnostic adapter Report callback is required".to_string());
    }
    let program = program.expect("program");
    let c = c.expect("checker");
    let sf = sf.expect("source file");
    let normalized = normalize_options(options, rule_name);
    let diagnostics = ets_rulerunner::run(
        ctx,
        program,
        c,
        sf,
        Some(&normalized),
        Some(&[rule_name]),
        ets_rulerunner::min_visible_severity(Some(&normalized)),
    )?;
    let resolved_options = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
        Some(&normalized),
        sf.file_name(),
        &program.options().config_file_path,
        program.use_case_sensitive_file_names(),
    );
    report_diagnostics(
        ctx,
        &diagnostics,
        rule_name,
        Some(program),
        Some(c),
        Some(sf),
        resolved_options.as_ref(),
        adapter,
    );
    Ok(())
}

// go: etsoxlintrunner/runner.go:127 reportDiagnostics 16685beec49a81ab
//
// Rust: Go's program, checker and source file may be nil when no suggestion is computed (its tests pass nil).
#[expect(clippy::too_many_arguments, reason = "Go's parameters, one for one")]
pub(crate) fn report_diagnostics(
    ctx: &Context,
    diagnostics: &[P<Diagnostic>],
    rule_name: &str,
    program: Option<&'static Program>,
    mut c: Option<&mut Checker>,
    sf: Option<P<SourceFile>>,
    options: Option<&ResolvedEffectPluginOptions>,
    mut adapter: DiagnosticAdapter<'_>,
) {
    for &diagnostic in diagnostics {
        let description = diagnostic_description(diagnostic, rule_name, &adapter);
        let related_information = reported_related_information(diagnostic, rule_name, &adapter);
        let c = c.as_deref_mut();
        let report = adapter.report.as_mut().expect("Report callback");
        report(ReportedDiagnostic {
            range: diagnostic_loc(diagnostic),
            message_id: format!("TS{}", diagnostic.code()),
            description,
            related_information,
            suggestions: Box::new(move || {
                reported_suggestions(ctx, program, c, sf, options, diagnostic)
            }),
        });
    }
}

/// Rust-only: Go's `diagnostic.Loc()`; tsrs stores an invalid location as `None`.
fn diagnostic_loc(diagnostic: P<Diagnostic>) -> TextRange {
    diagnostic.loc().unwrap_or_else(TextRange::synthetic)
}

// go: etsoxlintrunner/runner.go:150 reportedSuggestions 3ddf82616ddcaba9
//
// Rust: the nil program, checker or source file Go would dereference panic here.
pub(crate) fn reported_suggestions(
    ctx: &Context,
    program: Option<&'static Program>,
    c: Option<&mut Checker>,
    sf: Option<P<SourceFile>>,
    options: Option<&ResolvedEffectPluginOptions>,
    diagnostic: P<Diagnostic>,
) -> Vec<ReportedSuggestion> {
    let program = program.expect("program");
    let tp = ets_typeparser::new_type_parser(program, c.expect("checker"));
    let mut fix_ctx = ets_fixable::new_standalone_context(
        ctx,
        sf.expect("source file"),
        diagnostic_loc(diagnostic),
        diagnostic.code(),
        options,
        program,
        tp,
    );
    let mut suggestions = Vec::new();
    for provider in ets_fixables::by_error_code(diagnostic.code()) {
        if provider.name == "effectDisable" {
            continue;
        }
        for action in (provider.run)(&mut fix_ctx) {
            let edits = reported_edits(&fix_ctx, &action.changes);
            if edits.is_empty() {
                continue;
            }
            suggestions.push(ReportedSuggestion { description: action.description, edits });
        }
    }
    suggestions
}

// go: etsoxlintrunner/runner.go:179 reportedEdits 0aedd894a2a285a2
//
// Rust: tsrs's code actions hold their edits by value, so there is no nil edit to skip.
pub(crate) fn reported_edits(
    ctx: &ets_fixable::Context<'_, '_>,
    edits: &[lsproto::TextEdit],
) -> Vec<ReportedEdit> {
    let mut reported = Vec::with_capacity(edits.len());
    for edit in edits {
        reported.push(ReportedEdit {
            range: ctx.lsp_range_to_text_range(edit.range),
            text: edit.new_text.clone(),
        });
    }
    reported
}

// go: etsoxlintrunner/runner.go:193 diagnosticDescription b8a2c7b709a1b71c
pub(crate) fn diagnostic_description(
    diagnostic: P<Diagnostic>,
    rule_name: &str,
    adapter: &DiagnosticAdapter<'_>,
) -> String {
    let message = (adapter.message.expect("Message callback"))(diagnostic);
    if let Some(trimmed) = message.strip_suffix(&format!(" effect({rule_name})")) {
        return trimmed.to_string();
    }
    message
}

// go: etsoxlintrunner/runner.go:197 reportedRelatedInformation 622ef8b47bf1eb8a
//
// Rust: tsrs's related information holds no nil entries.
pub(crate) fn reported_related_information(
    diagnostic: P<Diagnostic>,
    rule_name: &str,
    adapter: &DiagnosticAdapter<'_>,
) -> Vec<ReportedRelatedInformation> {
    let related_diagnostics = diagnostic.related_information();
    if related_diagnostics.is_empty() {
        return Vec::new();
    }

    let mut related_information = Vec::with_capacity(related_diagnostics.len());
    for &related in related_diagnostics {
        let Some(file) = related.file() else {
            continue;
        };
        related_information.push(ReportedRelatedInformation {
            file_name: file.file_name().to_string(),
            range: diagnostic_loc(related),
            description: diagnostic_description(related, rule_name, adapter),
        });
    }
    related_information
}
