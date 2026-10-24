use std::sync::LazyLock;

use ets_etscore::{ResolvedEffectPluginOptions, Severity};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;
use rustc_hash::FxHashMap;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::Multiple_versions_of_package_0_were_detected_Colon_1_Package_duplication_can_change_runtime_identity_and_type_equality_across_Effect_modules_If_this_is_intentional_set_the_LSP_config_allowedDuplicatedPackages_to_2_effect_duplicatePackage;

// go: internal/rules/duplicate_package.go:18 duplicatePackageDiag
// duplicatePackageDiag holds pre-computed diagnostic info for a single duplicated package name.
pub(crate) struct DuplicatePackageDiag {
    pub(crate) package_name: String,
    pub(crate) details: String, // e.g. "1.0.0 @ /path/a, 2.0.0 @ /path/b"
    pub(crate) config_value: String,
}

// go: internal/rules/duplicate_package.go:26 DuplicatePackage
// DuplicatePackage warns when multiple versions of the same Effect-related package
// are loaded into the program.
pub static DUPLICATE_PACKAGE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "duplicatePackage",
    group: "correctness",
    description: "Warns when multiple versions of an Effect-related package are detected in the program",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let options = ctx.options;
        let entries = compute_duplicate_package_diags(ctx.type_parser, options);
        if entries.is_empty() {
            return Vec::new();
        }

        // Attach one diagnostic per duplicated package to the first statement (or the source file node).
        let stmts = ctx.source_file.statements().nodes();
        let target: P<Node> =
            if let Some(&first) = stmts.first() { first } else { ctx.source_file.as_node() };
        let loc = ctx.get_error_range(target);

        let mut diags = Vec::with_capacity(entries.len());
        for e in &entries {
            diags.push(ctx.new_diagnostic(
                ctx.source_file,
                loc,
                MESSAGE,
                &[],
                &[&e.package_name, &e.details, &e.config_value],
            ));
        }
        diags
    },
});

// go: internal/rules/duplicate_package.go:66 computeDuplicatePackageDiags
// computeDuplicatePackageDiags scans all packages and finds names with multiple distinct versions.
//
// Rust: Go's checker argument is unused and dropped.
pub(crate) fn compute_duplicate_package_diags(
    tp: &mut TypeParser<'_>,
    effect_config: Option<&ResolvedEffectPluginOptions>,
) -> Vec<DuplicatePackageDiag> {
    let packages = tp.discover_packages();

    // Filter to Effect-related packages.
    struct VersionEntry {
        version: String,
        dir: String,
    }
    let mut by_name: FxHashMap<String, Vec<VersionEntry>> = FxHashMap::default();
    for (_, pkg) in ets_readonly::new(packages.as_deref()).all() {
        if pkg.name() != "effect" && !pkg.depends_on_effect() {
            continue;
        }
        let ver = pkg.version().unwrap_or("");
        by_name.entry(pkg.name().to_string()).or_default().push(VersionEntry {
            version: ver.to_string(),
            dir: pkg.package_directory().to_string(),
        });
    }

    let allowed: &[String] =
        ResolvedEffectPluginOptions::get_allowed_duplicated_packages(effect_config);

    let mut diags = Vec::new();

    // Sort names for deterministic ordering.
    let mut names: Vec<String> = by_name.keys().cloned().collect();
    ets_go::slices::sort(&mut names);

    for name in &names {
        let entries = &by_name[name];
        if entries.len() <= 1 {
            continue;
        }
        if allowed.contains(name) {
            continue;
        }

        // Build details string: "1.0.0 @ /path/a, 2.0.0 @ /path/b"
        let mut parts = Vec::with_capacity(entries.len());
        for e in entries {
            if !e.version.is_empty() {
                parts.push(format!("{} @ {}", e.version, e.dir));
            } else {
                parts.push("(unknown) @ ".to_string() + &e.dir);
            }
        }
        let mut config_list: Vec<String> = allowed.to_vec();
        config_list.push(name.clone());
        let config_value = go_json_marshal_strings(&config_list);
        diags.push(DuplicatePackageDiag {
            package_name: name.clone(),
            details: parts.join(", "),
            config_value,
        });
    }

    diags
}

/// Rust-only: Go's `json.Marshal` of a `[]string` (cannot fail), with encoding/json's HTML-safe escaping of `<`,
/// `>`, `&` and U+2028/U+2029, which `serde_json` does not apply.
fn go_json_marshal_strings(values: &[String]) -> String {
    let encoded = serde_json::to_string(values).expect("a string list always encodes");
    encoded
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}
