// Port of etscore/options_test.go (package etscore_test).

use ets_etscore::{
    AnyValue, EFFECT_PLUGIN_NAME, EffectPluginOptions, Override, OverrideOptions, Severity,
    TopLevelNamedReexportsMode, parse_from_plugins,
};
use ets_go::sprintf;
use rustc_hash::FxHashMap;

/// Rust-only: the Go `any` values these tests build (`*collections.OrderedMap[string, any]`, `[]any`, `float64`,
/// Go `int`, `string`, `bool`).
#[derive(Clone, Debug)]
enum Any {
    Bool(bool),
    Float64(f64),
    Int(#[expect(dead_code, reason = "only its type matters: a Go int is not a float64")] i64),
    String(String),
    Slice(Vec<Any>),
    OrderedMap(Vec<(String, Any)>),
}

impl AnyValue for Any {
    fn is_nil(&self) -> bool {
        false
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Any::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_float64(&self) -> Option<f64> {
        match self {
            Any::Float64(f) => Some(*f),
            _ => None,
        }
    }

    fn as_string(&self) -> Option<&str> {
        match self {
            Any::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_slice(&self) -> Option<&[Self]> {
        match self {
            Any::Slice(items) => Some(items),
            _ => None,
        }
    }

    fn as_string_any_map(&self) -> Option<Vec<(&str, &Self)>> {
        match self {
            Any::OrderedMap(entries) => {
                Some(entries.iter().map(|(k, v)| (k.as_str(), v)).collect())
            }
            _ => None,
        }
    }
}

fn s(value: &str) -> Any {
    Any::String(value.to_string())
}

fn strs(values: &[&str]) -> Any {
    Any::Slice(values.iter().map(|value| s(value)).collect())
}

fn string_vec(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn severity_map(rule: &str, severity: Severity) -> FxHashMap<String, Severity> {
    let mut map = FxHashMap::default();
    map.insert(rule.to_string(), severity);
    map
}

fn string_map(entries: &[(&str, &str)]) -> FxHashMap<String, String> {
    entries.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
}

// go test: etscore/options_test.go:13 makePluginMap 70311bc4e7db18cc
// makePluginMap creates an *OrderedMap for testing with the given key-value pairs.
fn make_plugin_map(pairs: Vec<(&str, Any)>) -> Any {
    let mut m: Vec<(String, Any)> = Vec::new();
    for (key, value) in pairs {
        // OrderedMap.Set replaces an existing key in place.
        if let Some(entry) = m.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value;
        } else {
            m.push((key.to_string(), value));
        }
    }
    Any::OrderedMap(m)
}

// go test: etscore/options_test.go:22 makePlugins 228ef9913d1fd74a
// makePlugins wraps plugin maps into the []any structure expected by ParseFromPlugins.
fn make_plugins(maps: Vec<Any>) -> Any {
    Any::Slice(maps)
}

// go test: etscore/options_test.go:30 TestParseFromPlugins_ExitCodeDefaults 56d400f9859c883d
#[test]
fn test_parse_from_plugins_exit_code_defaults() {
    ets_go::testing::run(|t| {
        // Plugin with just the name — no exit-code keys present
        let plugins = make_plugins(vec![make_plugin_map(vec![("name", s(EFFECT_PLUGIN_NAME))])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.ignore_effect_suggestions_in_tsc_exit_code {
            t.error("expected IgnoreEffectSuggestionsInTscExitCode to default to true");
        }
        if opts.ignore_effect_warnings_in_tsc_exit_code {
            t.error("expected IgnoreEffectWarningsInTscExitCode to default to false");
        }
    });
}

// go test: etscore/options_test.go:45 TestParseFromPlugins_Overrides bc120282b40a3354
#[test]
fn test_parse_from_plugins_overrides() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            (
                "overrides",
                Any::Slice(vec![make_plugin_map(vec![
                    ("include", strs(&["src/**/*.ts"])),
                    ("exclude", strs(&["src/**/*.test.ts"])),
                    (
                        "options",
                        make_plugin_map(vec![
                            (
                                "diagnosticSeverity",
                                make_plugin_map(vec![("deterministicKeys", s("warning"))]),
                            ),
                            ("pipeableMinArgCount", Any::Float64(3.0)),
                            ("extendedKeyDetection", Any::Bool(true)),
                            ("allowedDuplicatedPackages", strs(&["effect"])),
                            ("allowedUnstableApis", strs(&["effect/http"])),
                            ("allowedExperimentalApis", strs(&["effect/schema/SchemaJITCompiler"])),
                        ]),
                    ),
                ])]),
            ),
        ])]);

        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        let overrides = opts.overrides.as_deref().unwrap_or_default();
        if overrides.len() != 1 {
            t.fatal(sprintf!("Overrides length = %d, want 1", overrides.len()));
        }
        let override_ = &overrides[0];
        let want = string_vec(&["src/**/*.ts"]);
        if override_.include.as_ref() != Some(&want) {
            t.error(sprintf!(
                "Include = %v, want %v",
                override_.include.clone().unwrap_or_default(),
                want
            ));
        }
        let want = string_vec(&["src/**/*.test.ts"]);
        if override_.exclude.as_ref() != Some(&want) {
            t.error(sprintf!(
                "Exclude = %v, want %v",
                override_.exclude.clone().unwrap_or_default(),
                want
            ));
        }
        let got = override_
            .options
            .diagnostic_severity
            .as_ref()
            .and_then(|m| m.get("deterministicKeys").copied())
            .unwrap_or_default();
        if got != Severity::Warning {
            t.error(sprintf!(
                "Diagnostics[deterministicKeys] = %v, want %v",
                got,
                Severity::Warning
            ));
        }
        if override_.options.pipeable_min_arg_count != Some(3) {
            t.fatal(sprintf!(
                "PipeableMinArgCount = %v, want 3",
                format!("{:?}", override_.options.pipeable_min_arg_count)
            ));
        }
        if override_.options.extended_key_detection != Some(true) {
            t.fatal(sprintf!(
                "ExtendedKeyDetection = %v, want true",
                format!("{:?}", override_.options.extended_key_detection)
            ));
        }
        if override_.options.allowed_unstable_apis != Some(string_vec(&["effect/http"])) {
            t.fatal(sprintf!(
                "AllowedUnstableApis = %v, want [effect/http]",
                format!("{:?}", override_.options.allowed_unstable_apis)
            ));
        }
        if override_.options.allowed_experimental_apis
            != Some(string_vec(&["effect/schema/SchemaJITCompiler"]))
        {
            t.fatal(sprintf!(
                "AllowedExperimentalApis = %v, want [effect/schema/SchemaJITCompiler]",
                format!("{:?}", override_.options.allowed_experimental_apis)
            ));
        }
        if override_.options.allowed_duplicated_packages != Some(string_vec(&["effect"])) {
            t.fatal(sprintf!(
                "AllowedDuplicatedPackages = %v, want [effect]",
                format!("{:?}", override_.options.allowed_duplicated_packages)
            ));
        }
    });
}

// go test: etscore/options_test.go:100 TestResolveForFile_AppliesMatchingScopesInOrder 1c9841dcb2e88df1
#[test]
fn test_resolve_for_file_applies_matching_scopes_in_order() {
    ets_go::testing::run(|t| {
        let opts = EffectPluginOptions {
            diagnostics: true,
            diagnostic_severity: Some(severity_map("deterministicKeys", Severity::Error)),
            pipeable_min_arg_count: 2,
            overrides: Some(vec![
                Override {
                    include: Some(string_vec(&["src/**/*.ts"])),
                    options: OverrideOptions {
                        diagnostic_severity: Some(severity_map(
                            "deterministicKeys",
                            Severity::Warning,
                        )),
                        pipeable_min_arg_count: Some(int_ptr(3)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Override {
                    include: Some(string_vec(&["src/legacy/**/*.ts"])),
                    options: OverrideOptions {
                        diagnostic_severity: Some(severity_map("deterministicKeys", Severity::Off)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        let resolved_severity = ets_pluginoptions::resolve_diagnostic_severity_for_file(
            Some(&opts),
            "/repo/src/legacy/example.ts",
            "/repo/tsconfig.json",
            true,
        );
        let effective = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
            Some(&opts),
            "/repo/src/legacy/example.ts",
            "/repo/tsconfig.json",
            true,
        );
        let got =
            resolved_severity.and_then(|m| m.get("deterministicKeys").copied()).unwrap_or_default();
        if got != Severity::Off {
            t.error(sprintf!("deterministicKeys severity = %v, want %v", got, Severity::Off));
        }
        let got = ets_etscore::ResolvedEffectPluginOptions::get_pipeable_min_arg_count(
            effective.as_ref(),
        );
        if got != 3 {
            t.error(sprintf!("PipeableMinArgCount = %d, want 3", got));
        }
    });
}

// go test: etscore/options_test.go:132 TestParseFromPlugins_OverrideImportStyle 988a8cbfdc510d04
#[test]
fn test_parse_from_plugins_override_import_style() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            (
                "overrides",
                Any::Slice(vec![make_plugin_map(vec![
                    ("include", strs(&["src/legacy/**/*.ts"])),
                    (
                        "options",
                        make_plugin_map(vec![
                            ("namespaceImportPackages", Any::Slice(vec![])),
                            ("barrelImportPackages", strs(&[" Effect "])),
                            (
                                "importAliases",
                                make_plugin_map(vec![("@Effect/Platform", s("Platform"))]),
                            ),
                            ("topLevelNamedReexports", s("FOLLOW")),
                        ]),
                    ),
                ])]),
            ),
        ])]);

        let opts = parse_from_plugins(&plugins);
        let Some(overrides) =
            opts.as_ref().and_then(|opts| opts.overrides.as_ref()).filter(|o| o.len() == 1)
        else {
            t.fatal(sprintf!("unexpected options: %+v", format!("{opts:?}")));
        };
        let options = &overrides[0].options;
        if options.namespace_import_packages.as_ref().is_none_or(|p| !p.is_empty()) {
            t.error(sprintf!(
                "NamespaceImportPackages = %v, want empty non-nil",
                format!("{:?}", options.namespace_import_packages)
            ));
        }
        if options.barrel_import_packages != Some(string_vec(&["effect"])) {
            t.error(sprintf!(
                "BarrelImportPackages = %v, want [effect]",
                format!("{:?}", options.barrel_import_packages)
            ));
        }
        if options.import_aliases != Some(string_map(&[("@effect/platform", "Platform")])) {
            t.error(sprintf!(
                "ImportAliases = %v, want map[@effect/platform:Platform]",
                format!("{:?}", options.import_aliases)
            ));
        }
        if options.top_level_named_reexports != Some(TopLevelNamedReexportsMode::Follow) {
            t.error(sprintf!(
                "TopLevelNamedReexports = %v, want follow",
                format!("{:?}", options.top_level_named_reexports)
            ));
        }
    });
}

// go test: etscore/options_test.go:167 TestResolveForFile_OverridesImportStyle d71c2b39a26659d6
#[test]
fn test_resolve_for_file_overrides_import_style() {
    ets_go::testing::run(|t| {
        let follow = TopLevelNamedReexportsMode::Follow;
        let opts = EffectPluginOptions {
            diagnostics: true,
            namespace_import_packages: Some(string_vec(&["effect"])),
            import_aliases: Some(string_map(&[("effect", "E")])),
            overrides: Some(vec![Override {
                include: Some(string_vec(&["src/legacy/**/*.ts"])),
                options: OverrideOptions {
                    namespace_import_packages: Some(Vec::new()),
                    barrel_import_packages: Some(string_vec(&["effect"])),
                    import_aliases: Some(FxHashMap::default()),
                    top_level_named_reexports: Some(follow),
                    ..Default::default()
                },
                ..Default::default()
            }]),
            ..Default::default()
        };

        let legacy = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
            Some(&opts),
            "/repo/src/legacy/example.ts",
            "/repo/tsconfig.json",
            true,
        )
        .unwrap();
        if !legacy.namespace_import_packages.is_empty() {
            t.error(sprintf!(
                "legacy NamespaceImportPackages = %v, want []",
                legacy.namespace_import_packages
            ));
        }
        if legacy.barrel_import_packages != string_vec(&["effect"]) {
            t.error(sprintf!(
                "legacy BarrelImportPackages = %v, want [effect]",
                legacy.barrel_import_packages
            ));
        }
        if !legacy.import_aliases.is_empty() {
            t.error(sprintf!(
                "legacy ImportAliases = %v, want empty",
                format!("{:?}", legacy.import_aliases)
            ));
        }
        if legacy.top_level_named_reexports != TopLevelNamedReexportsMode::Follow {
            t.error(sprintf!(
                "legacy TopLevelNamedReexports = %q, want follow",
                legacy.top_level_named_reexports.0.as_ref()
            ));
        }

        let other = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
            Some(&opts),
            "/repo/src/app/example.ts",
            "/repo/tsconfig.json",
            true,
        )
        .unwrap();
        if other.namespace_import_packages != string_vec(&["effect"]) {
            t.error(sprintf!(
                "other NamespaceImportPackages = %v, want [effect]",
                other.namespace_import_packages
            ));
        }
        if !other.barrel_import_packages.is_empty() {
            t.error(sprintf!(
                "other BarrelImportPackages = %v, want []",
                other.barrel_import_packages
            ));
        }
        if other.import_aliases != string_map(&[("effect", "E")]) {
            t.error(sprintf!(
                "other ImportAliases = %v, want map[effect:E]",
                format!("{:?}", other.import_aliases)
            ));
        }
        if other.top_level_named_reexports == TopLevelNamedReexportsMode::Follow {
            t.error("other TopLevelNamedReexports = follow, want unset");
        }
    });
}

// go test: etscore/options_test.go:215 TestResolveForFile_ExcludeOnlyAffectsCurrentScope d7035cbd5c198e73
#[test]
fn test_resolve_for_file_exclude_only_affects_current_scope() {
    ets_go::testing::run(|t| {
        let opts = EffectPluginOptions {
            diagnostics: true,
            diagnostic_severity: Some(severity_map("deterministicKeys", Severity::Error)),
            overrides: Some(vec![
                Override {
                    exclude: Some(string_vec(&["generated/**/*.ts"])),
                    options: OverrideOptions {
                        diagnostic_severity: Some(severity_map(
                            "deterministicKeys",
                            Severity::Warning,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Override {
                    include: Some(string_vec(&["generated/**/*.ts"])),
                    options: OverrideOptions {
                        diagnostic_severity: Some(severity_map("deterministicKeys", Severity::Off)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        let resolved_severity = ets_pluginoptions::resolve_diagnostic_severity_for_file(
            Some(&opts),
            "/repo/generated/example.ts",
            "/repo/tsconfig.json",
            true,
        );
        let got =
            resolved_severity.and_then(|m| m.get("deterministicKeys").copied()).unwrap_or_default();
        if got != Severity::Off {
            t.error(sprintf!("deterministicKeys severity = %v, want %v", got, Severity::Off));
        }
    });
}

// go test: etscore/options_test.go:241 intPtr 5a9dc238a4837eb5
fn int_ptr(value: i64) -> i64 {
    value
}

// go test: etscore/options_test.go:245 TestParseFromPlugins_ExitCodeExplicitTrue 5fc34a4c002b8bdb
#[test]
fn test_parse_from_plugins_exit_code_explicit_true() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("ignoreEffectSuggestionsInTscExitCode", Any::Bool(true)),
            ("ignoreEffectWarningsInTscExitCode", Any::Bool(true)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.ignore_effect_suggestions_in_tsc_exit_code {
            t.error("expected IgnoreEffectSuggestionsInTscExitCode to be true");
        }
        if !opts.ignore_effect_warnings_in_tsc_exit_code {
            t.error("expected IgnoreEffectWarningsInTscExitCode to be true");
        }
    });
}

// go test: etscore/options_test.go:263 TestParseFromPlugins_ExitCodeExplicitFalse 5dc90794f749a36c
#[test]
fn test_parse_from_plugins_exit_code_explicit_false() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("ignoreEffectSuggestionsInTscExitCode", Any::Bool(false)),
            ("ignoreEffectWarningsInTscExitCode", Any::Bool(false)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if opts.ignore_effect_suggestions_in_tsc_exit_code {
            t.error("expected IgnoreEffectSuggestionsInTscExitCode to be false");
        }
        if opts.ignore_effect_warnings_in_tsc_exit_code {
            t.error("expected IgnoreEffectWarningsInTscExitCode to be false");
        }
    });
}

// go test: etscore/options_test.go:281 TestParseFromPlugins_ExitCodeNonBooleanFallsBackToDefault 64c32a6373e1c311
#[test]
fn test_parse_from_plugins_exit_code_non_boolean_falls_back_to_default() {
    ets_go::testing::run(|t| {
        // Non-boolean values should fall back to defaults
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("ignoreEffectSuggestionsInTscExitCode", s("yes")),
            ("ignoreEffectWarningsInTscExitCode", Any::Int(42)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.ignore_effect_suggestions_in_tsc_exit_code {
            t.error("expected IgnoreEffectSuggestionsInTscExitCode to default to true for non-boolean value");
        }
        if opts.ignore_effect_warnings_in_tsc_exit_code {
            t.error("expected IgnoreEffectWarningsInTscExitCode to default to false for non-boolean value");
        }
    });
}

// go test: etscore/options_test.go:300 TestParseFromPlugins_IncludeSuggestionsInTscDefault 93da522070f1e979
#[test]
fn test_parse_from_plugins_include_suggestions_in_tsc_default() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![("name", s(EFFECT_PLUGIN_NAME))])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.include_suggestions_in_tsc {
            t.error("expected IncludeSuggestionsInTsc to default to true");
        }
        if !opts.refactors {
            t.error("expected Refactors to default to true");
        }
        if !opts.diagnostics {
            t.error("expected Diagnostics to default to true");
        }
        if !opts.quickinfo {
            t.error("expected Quickinfo to default to true");
        }
        if !opts.completions {
            t.error("expected Completions to default to true");
        }
        if !opts.goto {
            t.error("expected Goto to default to true");
        }
        if !opts.renames {
            t.error("expected Renames to default to true");
        }
    });
}

// go test: etscore/options_test.go:329 TestParseFromPlugins_FeatureFlagsExplicitFalse a5e704bb57446caf
#[test]
fn test_parse_from_plugins_feature_flags_explicit_false() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("refactors", Any::Bool(false)),
            ("diagnostics", Any::Bool(false)),
            ("quickinfo", Any::Bool(false)),
            ("completions", Any::Bool(false)),
            ("goto", Any::Bool(false)),
            ("renames", Any::Bool(false)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if opts.refactors {
            t.error("expected Refactors to be false");
        }
        if opts.diagnostics {
            t.error("expected Diagnostics to be false");
        }
        if opts.quickinfo {
            t.error("expected Quickinfo to be false");
        }
        if opts.completions {
            t.error("expected Completions to be false");
        }
        if opts.goto {
            t.error("expected Goto to be false");
        }
        if opts.renames {
            t.error("expected Renames to be false");
        }
    });
}

// go test: etscore/options_test.go:363 TestParseFromPlugins_IncludeSuggestionsInTscExplicitTrue d8e6b192a61c67e0
#[test]
fn test_parse_from_plugins_include_suggestions_in_tsc_explicit_true() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("includeSuggestionsInTsc", Any::Bool(true)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.include_suggestions_in_tsc {
            t.error("expected IncludeSuggestionsInTsc to be true");
        }
    });
}

// go test: etscore/options_test.go:377 TestParseFromPlugins_IncludeSuggestionsInTscExplicitFalse a9a32b010e84562d
#[test]
fn test_parse_from_plugins_include_suggestions_in_tsc_explicit_false() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("includeSuggestionsInTsc", Any::Bool(false)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if opts.include_suggestions_in_tsc {
            t.error("expected IncludeSuggestionsInTsc to be false");
        }
    });
}

// go test: etscore/options_test.go:391 TestParseFromPlugins_IncludeSuggestionsInTscNonBooleanFallback dd551d56ee2e31b1
#[test]
fn test_parse_from_plugins_include_suggestions_in_tsc_non_boolean_fallback() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("includeSuggestionsInTsc", s("yes")),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.include_suggestions_in_tsc {
            t.error("expected IncludeSuggestionsInTsc to default to true for non-boolean value");
        }
    });
}

// go test: etscore/options_test.go:405 TestGetIncludeSuggestionsInTsc_NilReceiver 4221e0348dbf27ba
#[test]
fn test_get_include_suggestions_in_tsc_nil_receiver() {
    ets_go::testing::run(|t| {
        let opts: Option<&EffectPluginOptions> = None;
        if !EffectPluginOptions::get_include_suggestions_in_tsc(opts) {
            t.error("expected GetIncludeSuggestionsInTsc() on nil to return true");
        }
    });
}

// go test: etscore/options_test.go:412 TestGetMermaidBaseURL bca0980f553d6cf9
#[test]
fn test_get_mermaid_base_url() {
    ets_go::testing::run(|t| {
        let tests: &[(&str, &str, &str)] = &[
            ("default empty", "", "https://mermaid.live/edit#"),
            ("mermaid.live", "mermaid.live", "https://mermaid.live/edit#"),
            ("mermaid.com", "mermaid.com", "https://www.mermaidchart.com/play#"),
            (
                "custom URL",
                "https://my-mermaid.example.com",
                "https://my-mermaid.example.com/edit#",
            ),
        ];
        for &(name, provider, want) in tests {
            t.run(name, |t| {
                let opts = EffectPluginOptions {
                    mermaid_provider: provider.to_string(),
                    ..Default::default()
                };
                let got = EffectPluginOptions::get_mermaid_base_url(Some(&opts));
                if got != want {
                    t.error(sprintf!("GetMermaidBaseURL() = %q, want %q", got, want));
                }
            });
        }
    });
}

// go test: etscore/options_test.go:433 TestGetMermaidBaseURL_Nil 6c14dc7518092e9e
#[test]
fn test_get_mermaid_base_url_nil() {
    ets_go::testing::run(|t| {
        let opts: Option<&EffectPluginOptions> = None;
        let got = EffectPluginOptions::get_mermaid_base_url(opts);
        if got != "https://mermaid.live/edit#" {
            t.error(sprintf!("GetMermaidBaseURL() on nil = %q, want default", got));
        }
    });
}

// go test: etscore/options_test.go:440 TestGetLayerGraphFollowDepth bd7784769f8e86cf
#[test]
fn test_get_layer_graph_follow_depth() {
    ets_go::testing::run(|t| {
        // Default (zero value)
        let mut opts = EffectPluginOptions::default();
        let got = EffectPluginOptions::get_layer_graph_follow_depth(Some(&opts));
        if got != 0 {
            t.error(sprintf!("GetLayerGraphFollowDepth() = %d, want 0", got));
        }

        // Non-zero
        opts.layer_graph_follow_depth = 3;
        let got = EffectPluginOptions::get_layer_graph_follow_depth(Some(&opts));
        if got != 3 {
            t.error(sprintf!("GetLayerGraphFollowDepth() = %d, want 3", got));
        }

        // Nil receiver
        let nil_opts: Option<&EffectPluginOptions> = None;
        let got = EffectPluginOptions::get_layer_graph_follow_depth(nil_opts);
        if got != 0 {
            t.error(sprintf!("GetLayerGraphFollowDepth() on nil = %d, want 0", got));
        }
    });
}

// go test: etscore/options_test.go:460 TestParseFromPlugins_MermaidProvider 21d7f846eb0fca07
#[test]
fn test_parse_from_plugins_mermaid_provider() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("mermaidProvider", s("mermaid.com")),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if opts.mermaid_provider != "mermaid.com" {
            t.error(sprintf!(
                "MermaidProvider = %q, want %q",
                opts.mermaid_provider,
                "mermaid.com"
            ));
        }
    });
}

// go test: etscore/options_test.go:474 TestParseFromPlugins_NoExternal e16492a1169c108e
#[test]
fn test_parse_from_plugins_no_external() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("noExternal", Any::Bool(true)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.no_external {
            t.error("expected NoExternal to be true");
        }
    });
}

// go test: etscore/options_test.go:488 TestParseFromPlugins_LayerGraphFollowDepth 4cdbe99c7d1d78a6
#[test]
fn test_parse_from_plugins_layer_graph_follow_depth() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("layerGraphFollowDepth", Any::Float64(2.0)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if opts.layer_graph_follow_depth != 2 {
            t.error(sprintf!("LayerGraphFollowDepth = %d, want 2", opts.layer_graph_follow_depth));
        }
    });
}

// go test: etscore/options_test.go:502 TestParseFromPlugins_NewOptionsDefaults 4710ca7fac454c1d
#[test]
fn test_parse_from_plugins_new_options_defaults() {
    ets_go::testing::run(|t| {
        // Plugin with just the name — new options should have zero-value defaults
        let plugins = make_plugins(vec![make_plugin_map(vec![("name", s(EFFECT_PLUGIN_NAME))])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if !opts.mermaid_provider.is_empty() {
            t.error(sprintf!("MermaidProvider = %q, want empty", opts.mermaid_provider));
        }
        if opts.no_external {
            t.error("expected NoExternal to default to false");
        }
        if opts.layer_graph_follow_depth != 0 {
            t.error(sprintf!("LayerGraphFollowDepth = %d, want 0", opts.layer_graph_follow_depth));
        }
        if let Some(allowed) = &opts.allowed_duplicated_packages {
            t.error(sprintf!("AllowedDuplicatedPackages = %v, want nil", allowed));
        }
    });
}

// go test: etscore/options_test.go:523 TestParseFromPlugins_AllowedDuplicatedPackages 36bdb57c9556fbc2
#[test]
fn test_parse_from_plugins_allowed_duplicated_packages() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("allowedDuplicatedPackages", strs(&["effect", "@effect/platform"])),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        let got = opts.allowed_duplicated_packages.unwrap_or_default();
        if got.len() != 2 {
            t.fatal(sprintf!("AllowedDuplicatedPackages length = %d, want 2", got.len()));
        }
        if got[0] != "effect" {
            t.error(sprintf!("AllowedDuplicatedPackages[0] = %q, want %q", got[0], "effect"));
        }
        if got[1] != "@effect/platform" {
            t.error(sprintf!(
                "AllowedDuplicatedPackages[1] = %q, want %q",
                got[1],
                "@effect/platform"
            ));
        }
    });
}

// go test: etscore/options_test.go:544 TestAutoImportStyleDefaults 0246f3db57a4128c
#[test]
fn test_auto_import_style_defaults() {
    ets_go::testing::run(|t| {
        let opts = EffectPluginOptions::default();

        let got = EffectPluginOptions::get_namespace_import_packages(Some(&opts));
        if !got.is_empty() {
            t.error(sprintf!("GetNamespaceImportPackages() = %v, want empty list", got));
        }
        let got = EffectPluginOptions::get_barrel_import_packages(Some(&opts));
        if !got.is_empty() {
            t.error(sprintf!("GetBarrelImportPackages() = %v, want empty list", got));
        }
        let got = EffectPluginOptions::get_import_aliases(Some(&opts));
        if !got.is_empty() {
            t.error(sprintf!("GetImportAliases() = %v, want empty map", format!("{got:?}")));
        }
        let got = EffectPluginOptions::get_top_level_named_reexports(Some(&opts));
        if got != TopLevelNamedReexportsMode::Ignore {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() = %q, want %q",
                got.0.as_ref(),
                "ignore"
            ));
        }
    });
}

// go test: etscore/options_test.go:561 TestAutoImportStyleNormalization 63ca2f760a58c0c1
#[test]
fn test_auto_import_style_normalization() {
    ets_go::testing::run(|t| {
        let opts = EffectPluginOptions {
            namespace_import_packages: Some(string_vec(&["Effect", " @Effect/Platform ", ""])),
            barrel_import_packages: Some(string_vec(&["@Effect/Schema", " EFFECT/SQL "])),
            import_aliases: Some(string_map(&[
                ("Effect", "E"),
                (" @Effect/Platform ", "Platform"),
                ("", "Invalid"),
            ])),
            top_level_named_reexports: TopLevelNamedReexportsMode("FOLLOW".into()),
            ..Default::default()
        };

        let (got, want) = (
            EffectPluginOptions::get_namespace_import_packages(Some(&opts)),
            string_vec(&["effect", "@effect/platform"]),
        );
        if got != want {
            t.error(sprintf!("GetNamespaceImportPackages() = %v, want %v", got, want));
        }
        let (got, want) = (
            EffectPluginOptions::get_barrel_import_packages(Some(&opts)),
            string_vec(&["@effect/schema", "effect/sql"]),
        );
        if got != want {
            t.error(sprintf!("GetBarrelImportPackages() = %v, want %v", got, want));
        }
        let (got, want) = (
            EffectPluginOptions::get_import_aliases(Some(&opts)),
            string_map(&[("effect", "E"), ("@effect/platform", "Platform")]),
        );
        if got != want {
            t.error(sprintf!(
                "GetImportAliases() = %v, want %v",
                format!("{got:?}"),
                format!("{want:?}")
            ));
        }
        let got = EffectPluginOptions::get_top_level_named_reexports(Some(&opts));
        if got != TopLevelNamedReexportsMode::Follow {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() = %q, want %q",
                got.0.as_ref(),
                "follow"
            ));
        }
    });
}

// go test: etscore/options_test.go:587 TestTopLevelNamedReexportsInvalidFallback da3d27566cd1b184
#[test]
fn test_top_level_named_reexports_invalid_fallback() {
    ets_go::testing::run(|t| {
        let opts = EffectPluginOptions {
            top_level_named_reexports: TopLevelNamedReexportsMode("invalid".into()),
            ..Default::default()
        };
        let got = EffectPluginOptions::get_top_level_named_reexports(Some(&opts));
        if got != TopLevelNamedReexportsMode::Ignore {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() = %q, want %q",
                got.0.as_ref(),
                "ignore"
            ));
        }
    });
}

// go test: etscore/options_test.go:594 TestParseFromPlugins_AutoImportStyleDefaults 0f2ff8e275cf69cb
#[test]
fn test_parse_from_plugins_auto_import_style_defaults() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![("name", s(EFFECT_PLUGIN_NAME))])]);
        let opts = parse_from_plugins(&plugins);
        if opts.is_none() {
            t.fatal("expected non-nil options");
        }
        let opts = opts.as_ref();

        let got = EffectPluginOptions::get_namespace_import_packages(opts);
        if !got.is_empty() {
            t.error(sprintf!("GetNamespaceImportPackages() = %v, want empty list", got));
        }
        let got = EffectPluginOptions::get_barrel_import_packages(opts);
        if !got.is_empty() {
            t.error(sprintf!("GetBarrelImportPackages() = %v, want empty list", got));
        }
        let got = EffectPluginOptions::get_import_aliases(opts);
        if !got.is_empty() {
            t.error(sprintf!("GetImportAliases() = %v, want empty map", format!("{got:?}")));
        }
        let got = EffectPluginOptions::get_top_level_named_reexports(opts);
        if got != TopLevelNamedReexportsMode::Ignore {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() = %q, want %q",
                got.0.as_ref(),
                "ignore"
            ));
        }
    });
}

// go test: etscore/options_test.go:615 TestParseFromPlugins_AutoImportStyleValidValues 16573f1de8b2de1c
#[test]
fn test_parse_from_plugins_auto_import_style_valid_values() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("namespaceImportPackages", strs(&["Effect", " @Effect/Platform "])),
            ("barrelImportPackages", strs(&["@Effect/Schema", " EFFECT/SQL "])),
            (
                "importAliases",
                make_plugin_map(vec![("Effect", s("E")), (" @Effect/Platform ", s("Platform"))]),
            ),
            ("topLevelNamedReexports", s("FOLLOW")),
        ])]);
        let opts = parse_from_plugins(&plugins);
        if opts.is_none() {
            t.fatal("expected non-nil options");
        }
        let opts = opts.as_ref();

        let (got, want) = (
            EffectPluginOptions::get_namespace_import_packages(opts),
            string_vec(&["effect", "@effect/platform"]),
        );
        if got != want {
            t.error(sprintf!("GetNamespaceImportPackages() = %v, want %v", got, want));
        }
        let (got, want) = (
            EffectPluginOptions::get_barrel_import_packages(opts),
            string_vec(&["@effect/schema", "effect/sql"]),
        );
        if got != want {
            t.error(sprintf!("GetBarrelImportPackages() = %v, want %v", got, want));
        }
        let (got, want) = (
            EffectPluginOptions::get_import_aliases(opts),
            string_map(&[("effect", "E"), ("@effect/platform", "Platform")]),
        );
        if got != want {
            t.error(sprintf!(
                "GetImportAliases() = %v, want %v",
                format!("{got:?}"),
                format!("{want:?}")
            ));
        }
        let got = EffectPluginOptions::get_top_level_named_reexports(opts);
        if got != TopLevelNamedReexportsMode::Follow {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() = %q, want %q",
                got.0.as_ref(),
                "follow"
            ));
        }
    });
}

// go test: etscore/options_test.go:645 TestParseFromPlugins_AutoImportStyleInvalidFallback 4385cb52d8b8c9ee
#[test]
fn test_parse_from_plugins_auto_import_style_invalid_fallback() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("namespaceImportPackages", Any::Slice(vec![s("effect"), Any::Int(1)])),
            ("barrelImportPackages", s("not-an-array")),
            (
                "importAliases",
                make_plugin_map(vec![("effect", s("E")), ("@effect/platform", Any::Int(1))]),
            ),
            ("topLevelNamedReexports", s("invalid")),
        ])]);
        let opts = parse_from_plugins(&plugins);
        if opts.is_none() {
            t.fatal("expected non-nil options");
        }
        let opts = opts.as_ref();

        let got = EffectPluginOptions::get_namespace_import_packages(opts);
        if !got.is_empty() {
            t.error(sprintf!(
                "GetNamespaceImportPackages() = %v, want empty list for mixed array fallback",
                got
            ));
        }
        let got = EffectPluginOptions::get_barrel_import_packages(opts);
        if !got.is_empty() {
            t.error(sprintf!(
                "GetBarrelImportPackages() = %v, want empty list for wrong-type fallback",
                got
            ));
        }
        let got = EffectPluginOptions::get_import_aliases(opts);
        if !got.is_empty() {
            t.error(sprintf!(
                "GetImportAliases() = %v, want empty map for mixed map fallback",
                format!("{got:?}")
            ));
        }
        let got = EffectPluginOptions::get_top_level_named_reexports(opts);
        if got != TopLevelNamedReexportsMode::Ignore {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() = %q, want %q for invalid enum fallback",
                got.0.as_ref(),
                "ignore"
            ));
        }
    });
}

// go test: etscore/options_test.go:675 TestParseFromPlugins_AutoImportStyleWrongTypes db8309d340eb3794
#[test]
fn test_parse_from_plugins_auto_import_style_wrong_types() {
    ets_go::testing::run(|t| {
        // All fields have completely wrong types (not arrays/maps/strings)
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("namespaceImportPackages", Any::Int(42)),
            ("barrelImportPackages", Any::Int(42)),
            ("importAliases", s("not-a-map")),
            ("topLevelNamedReexports", Any::Int(42)),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };

        if let Some(got) = &opts.namespace_import_packages {
            t.error(sprintf!(
                "NamespaceImportPackages = %v, want nil for wrong-type fallback",
                got
            ));
        }
        if let Some(got) = &opts.barrel_import_packages {
            t.error(sprintf!("BarrelImportPackages = %v, want nil for wrong-type fallback", got));
        }
        if let Some(got) = &opts.import_aliases {
            t.error(sprintf!(
                "ImportAliases = %v, want nil for wrong-type fallback",
                format!("{got:?}")
            ));
        }
        if !opts.top_level_named_reexports.0.is_empty() {
            t.error(sprintf!(
                "TopLevelNamedReexports = %q, want empty string for non-string fallback",
                opts.top_level_named_reexports.0.as_ref()
            ));
        }
    });
}

// go test: etscore/options_test.go:703 TestParseFromPlugins_TopLevelNamedReexportsIgnoreCase 59ba56cdeb4ad819
#[test]
fn test_parse_from_plugins_top_level_named_reexports_ignore_case() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("topLevelNamedReexports", s("IGNORE")),
        ])]);
        let Some(opts) = parse_from_plugins(&plugins) else {
            t.fatal("expected non-nil options");
        };
        if opts.top_level_named_reexports.0 != "ignore" {
            t.error(sprintf!(
                "TopLevelNamedReexports = %q, want %q (case-insensitive)",
                opts.top_level_named_reexports.0.as_ref(),
                "ignore"
            ));
        }
    });
}

// go test: etscore/options_test.go:717 TestAutoImportStyleGetters_NilReceiver 25cbcda4c8a64dec
#[test]
fn test_auto_import_style_getters_nil_receiver() {
    ets_go::testing::run(|t| {
        let opts: Option<&EffectPluginOptions> = None;

        let got = EffectPluginOptions::get_namespace_import_packages(opts);
        if !got.is_empty() {
            t.error(sprintf!("GetNamespaceImportPackages() on nil = %v, want empty list", got));
        }
        let got = EffectPluginOptions::get_barrel_import_packages(opts);
        if !got.is_empty() {
            t.error(sprintf!("GetBarrelImportPackages() on nil = %v, want empty list", got));
        }
        let got = EffectPluginOptions::get_import_aliases(opts);
        if !got.is_empty() {
            t.error(sprintf!("GetImportAliases() on nil = %v, want empty map", format!("{got:?}")));
        }
        let got = EffectPluginOptions::get_top_level_named_reexports(opts);
        if got != TopLevelNamedReexportsMode::Ignore {
            t.error(sprintf!(
                "GetTopLevelNamedReexports() on nil = %q, want %q",
                got.0.as_ref(),
                "ignore"
            ));
        }
    });
}

// go test: etscore/options_test.go:734 TestParseFromPlugins_AutoImportStyleNormalizationDropsEmptyEntries 7bf5f186a7cf9f23
#[test]
fn test_parse_from_plugins_auto_import_style_normalization_drops_empty_entries() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("namespaceImportPackages", strs(&["  ", "Effect", "\t"])),
            ("barrelImportPackages", strs(&["", " @Effect/Schema "])),
            (
                "importAliases",
                make_plugin_map(vec![
                    ("", s("Ignored")),
                    (" ", s("AlsoIgnored")),
                    (" Effect ", s("E")),
                ]),
            ),
        ])]);
        let opts = parse_from_plugins(&plugins);
        if opts.is_none() {
            t.fatal("expected non-nil options");
        }
        let opts = opts.as_ref();

        let (got, want) =
            (EffectPluginOptions::get_namespace_import_packages(opts), string_vec(&["effect"]));
        if got != want {
            t.error(sprintf!("GetNamespaceImportPackages() = %v, want %v", got, want));
        }
        let (got, want) = (
            EffectPluginOptions::get_barrel_import_packages(opts),
            string_vec(&["@effect/schema"]),
        );
        if got != want {
            t.error(sprintf!("GetBarrelImportPackages() = %v, want %v", got, want));
        }
        let (got, want) =
            (EffectPluginOptions::get_import_aliases(opts), string_map(&[("effect", "E")]));
        if got != want {
            t.error(sprintf!(
                "GetImportAliases() = %v, want %v",
                format!("{got:?}"),
                format!("{want:?}")
            ));
        }
    });
}

// go test: etscore/options_test.go:761 TestParseFromPlugins_AllowedUnstableApis 4f30b8c487065128
#[test]
fn test_parse_from_plugins_allowed_unstable_apis() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("allowedUnstableApis", strs(&["effect/http", "effect/rpc/Client#make"])),
        ])]);
        let opts = parse_from_plugins(&plugins);
        if opts.as_ref().is_none_or(|opts| {
            opts.allowed_unstable_apis
                != Some(string_vec(&["effect/http", "effect/rpc/Client#make"]))
        }) {
            t.fatal(sprintf!("unexpected options: %+v", format!("{opts:?}")));
        }
    });
}

// go test: etscore/options_test.go:772 TestParseFromPlugins_AllowedExperimentalApis c363cfc92a0515f5
#[test]
fn test_parse_from_plugins_allowed_experimental_apis() {
    ets_go::testing::run(|t| {
        let plugins = make_plugins(vec![make_plugin_map(vec![
            ("name", s(EFFECT_PLUGIN_NAME)),
            ("allowedExperimentalApis", strs(&["effect/http", "effect/rpc/Client#make"])),
        ])]);
        let opts = parse_from_plugins(&plugins);
        if opts.as_ref().is_none_or(|opts| {
            opts.allowed_experimental_apis
                != Some(string_vec(&["effect/http", "effect/rpc/Client#make"]))
        }) {
            t.fatal(sprintf!("unexpected options: %+v", format!("{opts:?}")));
        }
    });
}
