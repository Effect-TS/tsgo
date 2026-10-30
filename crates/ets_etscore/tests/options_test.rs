// Port of etscore/options_test.go (package etscore_test).

use ets_etscore::{
    AnyValue, DEFAULT_BRANDED_SCHEMA_CLASSES, EFFECT_PLUGIN_NAME, EffectPluginOptions, Override,
    OverrideOptions, ResolvedEffectPluginOptions, Severity, TopLevelNamedReexportsMode,
    parse_from_plugins,
};
use ets_go::sprintf;
use ets_go::testing::T;
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

// go test: etscore/options_test.go:12 makePluginMap 791bab80392447c5
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

// go test: etscore/options_test.go:20 makePlugins 04294b20c846fc11
fn make_plugins(maps: Vec<Any>) -> Any {
    Any::Slice(maps)
}

// go test: etscore/options_test.go:28 TestParseFromPlugins_Overrides bc120282b40a3354
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

// go test: etscore/options_test.go:83 TestResolveForFile_AppliesMatchingScopesInOrder 1c9841dcb2e88df1
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

// go test: etscore/options_test.go:115 TestParseFromPlugins_OverrideImportStyle 988a8cbfdc510d04
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

// go test: etscore/options_test.go:150 TestResolveForFile_OverridesImportStyle d71c2b39a26659d6
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

// go test: etscore/options_test.go:198 TestResolveForFile_ExcludeOnlyAffectsCurrentScope d7035cbd5c198e73
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

// go test: etscore/options_test.go:224 intPtr 5a9dc238a4837eb5
fn int_ptr(value: i64) -> i64 {
    value
}

// go test: etscore/options_test.go:228 parseNameOnlyPlugin 76eec2bff8d255c7
fn parse_name_only_plugin(t: &T, pairs: Vec<(&str, Any)>) -> EffectPluginOptions {
    t.helper();
    let mut all = vec![("name", s(EFFECT_PLUGIN_NAME))];
    all.extend(pairs);
    let Some(opts) = parse_from_plugins(&make_plugins(vec![make_plugin_map(all)])) else {
        t.fatal("expected non-nil options");
    };
    opts
}

// go test: etscore/options_test.go:237 TestParseFromPlugins_BooleanOptions 0bef9b530b081bc9
#[test]
fn test_parse_from_plugins_boolean_options() {
    ets_go::testing::run(|t| {
        type Field = fn(&EffectPluginOptions) -> bool;
        let tests: &[(&str, Field, bool)] = &[
            ("refactors", |o| o.refactors, true),
            ("diagnostics", |o| o.diagnostics, true),
            ("includeSuggestionsInTsc", |o| o.include_suggestions_in_tsc, true),
            ("quickinfo", |o| o.quickinfo, true),
            ("completions", |o| o.completions, true),
            ("goto", |o| o.goto, true),
            ("renames", |o| o.renames, true),
            (
                "ignoreEffectSuggestionsInTscExitCode",
                |o| o.ignore_effect_suggestions_in_tsc_exit_code,
                true,
            ),
            (
                "ignoreEffectWarningsInTscExitCode",
                |o| o.ignore_effect_warnings_in_tsc_exit_code,
                false,
            ),
            ("noExternal", |o| o.no_external, false),
        ];
        for &(key, field, default_val) in tests {
            t.run(key, |t| {
                let cases: Vec<(&str, Vec<(&str, Any)>, bool)> = vec![
                    ("omitted", vec![], default_val),
                    ("true", vec![(key, Any::Bool(true))], true),
                    ("false", vec![(key, Any::Bool(false))], false),
                    ("non-boolean", vec![(key, s("yes"))], default_val),
                ];
                for (name, pairs, want) in cases {
                    let got = field(&parse_name_only_plugin(t, pairs));
                    if got != want {
                        t.error(format!("{name} {key} = {got}, want {want}"));
                    }
                }
            });
        }
    });
}

// go test: etscore/options_test.go:273 TestParseFromPlugins_ValueOptions 7f762addc919d5fd
#[test]
fn test_parse_from_plugins_value_options() {
    ets_go::testing::run(|t| {
        let defaults = parse_name_only_plugin(t, vec![]);
        if !defaults.mermaid_provider.is_empty()
            || defaults.layer_graph_follow_depth != 0
            || defaults.allowed_duplicated_packages.is_some()
        {
            t.error(sprintf!(
                "unexpected defaults: MermaidProvider=%q LayerGraphFollowDepth=%d AllowedDuplicatedPackages=%v",
                defaults.mermaid_provider,
                defaults.layer_graph_follow_depth,
                defaults.allowed_duplicated_packages.clone().unwrap_or_default()
            ));
        }

        let opts = parse_name_only_plugin(
            t,
            vec![
                ("mermaidProvider", s("mermaid.com")),
                ("layerGraphFollowDepth", Any::Float64(2.0)),
                ("allowedDuplicatedPackages", strs(&["effect", "@effect/platform"])),
                ("allowedUnstableApis", strs(&["effect/http", "effect/rpc/Client#make"])),
                ("allowedExperimentalApis", strs(&["effect/http", "effect/rpc/Client#make"])),
            ],
        );
        if opts.mermaid_provider != "mermaid.com" {
            t.error(sprintf!(
                "MermaidProvider = %q, want %q",
                opts.mermaid_provider,
                "mermaid.com"
            ));
        }
        if opts.layer_graph_follow_depth != 2 {
            t.error(sprintf!("LayerGraphFollowDepth = %d, want 2", opts.layer_graph_follow_depth));
        }
        let want = string_vec(&["effect", "@effect/platform"]);
        if opts.allowed_duplicated_packages.as_ref() != Some(&want) {
            t.error(sprintf!(
                "AllowedDuplicatedPackages = %v, want %v",
                opts.allowed_duplicated_packages.clone().unwrap_or_default(),
                want
            ));
        }
        let want = string_vec(&["effect/http", "effect/rpc/Client#make"]);
        if opts.allowed_unstable_apis.as_ref() != Some(&want) {
            t.error(sprintf!(
                "AllowedUnstableApis = %v, want %v",
                opts.allowed_unstable_apis.clone().unwrap_or_default(),
                want
            ));
        }
        let want = string_vec(&["effect/http", "effect/rpc/Client#make"]);
        if opts.allowed_experimental_apis.as_ref() != Some(&want) {
            t.error(sprintf!(
                "AllowedExperimentalApis = %v, want %v",
                opts.allowed_experimental_apis.unwrap_or_default(),
                want
            ));
        }
    });
}

// go test: etscore/options_test.go:304 TestNilOptionsGetters 930a0132d3a79c14
#[test]
fn test_nil_options_getters() {
    ets_go::testing::run(|t| {
        let opts: Option<&EffectPluginOptions> = None;
        if !EffectPluginOptions::get_include_suggestions_in_tsc(opts) {
            t.error("expected GetIncludeSuggestionsInTsc() on nil to return true");
        }
        let got = EffectPluginOptions::get_mermaid_base_url(opts);
        if got != "https://mermaid.live/edit#" {
            t.error(sprintf!("GetMermaidBaseURL() on nil = %q, want default", got));
        }
        let got = EffectPluginOptions::get_layer_graph_follow_depth(opts);
        if got != 0 {
            t.error(sprintf!("GetLayerGraphFollowDepth() on nil = %d, want 0", got));
        }
    });
}

// go test: etscore/options_test.go:317 TestGetMermaidBaseURL bca0980f553d6cf9
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

// go test: etscore/options_test.go:338 assertImportStyle d1ebf9396626d320
fn assert_import_style(
    t: &T,
    opts: Option<&EffectPluginOptions>,
    namespace: &[String],
    barrel: &[String],
    aliases: &FxHashMap<String, String>,
    mode: &TopLevelNamedReexportsMode,
) {
    t.helper();
    let got = EffectPluginOptions::get_namespace_import_packages(opts);
    if got != namespace && (!got.is_empty() || !namespace.is_empty()) {
        t.error(sprintf!("GetNamespaceImportPackages() = %v, want %v", got, namespace.to_vec()));
    }
    let got = EffectPluginOptions::get_barrel_import_packages(opts);
    if got != barrel && (!got.is_empty() || !barrel.is_empty()) {
        t.error(sprintf!("GetBarrelImportPackages() = %v, want %v", got, barrel.to_vec()));
    }
    let got = EffectPluginOptions::get_import_aliases(opts);
    if got != *aliases && (!got.is_empty() || !aliases.is_empty()) {
        t.error(sprintf!(
            "GetImportAliases() = %v, want %v",
            format!("{got:?}"),
            format!("{aliases:?}")
        ));
    }
    let got = EffectPluginOptions::get_top_level_named_reexports(opts);
    if got != *mode {
        t.error(sprintf!(
            "GetTopLevelNamedReexports() = %q, want %q",
            got.0.as_ref(),
            mode.0.as_ref()
        ));
    }
}

// go test: etscore/options_test.go:354 TestAutoImportStyleFallsBackToDefaults e7273bbb2266c073
#[test]
fn test_auto_import_style_falls_back_to_defaults() {
    ets_go::testing::run(|t| {
        type Opts = fn(&T) -> Option<EffectPluginOptions>;
        let tests: &[(&str, Opts)] = &[
            ("nil receiver", |_| None),
            ("zero options", |_| Some(EffectPluginOptions::default())),
            ("invalid mode", |_| {
                Some(EffectPluginOptions {
                    top_level_named_reexports: TopLevelNamedReexportsMode("invalid".into()),
                    ..Default::default()
                })
            }),
            ("name-only plugin", |t| Some(parse_name_only_plugin(t, vec![]))),
            ("mixed or invalid values", |t| {
                Some(parse_name_only_plugin(
                    t,
                    vec![
                        ("namespaceImportPackages", Any::Slice(vec![s("effect"), Any::Int(1)])),
                        ("barrelImportPackages", s("not-an-array")),
                        (
                            "importAliases",
                            make_plugin_map(vec![
                                ("effect", s("E")),
                                ("@effect/platform", Any::Int(1)),
                            ]),
                        ),
                        ("topLevelNamedReexports", s("invalid")),
                    ],
                ))
            }),
            ("wrong types", |t| {
                Some(parse_name_only_plugin(
                    t,
                    vec![
                        ("namespaceImportPackages", Any::Int(42)),
                        ("barrelImportPackages", Any::Int(42)),
                        ("importAliases", s("not-a-map")),
                        ("topLevelNamedReexports", Any::Int(42)),
                    ],
                ))
            }),
        ];
        for &(name, opts) in tests {
            t.run(name, |t| {
                assert_import_style(
                    t,
                    opts(t).as_ref(),
                    &[],
                    &[],
                    &FxHashMap::default(),
                    &TopLevelNamedReexportsMode::Ignore,
                );
            });
        }
    });
}

// go test: etscore/options_test.go:388 TestAutoImportStyleNormalization 97482aab14563609
#[test]
fn test_auto_import_style_normalization() {
    ets_go::testing::run(|t| {
        let namespace = string_vec(&["effect", "@effect/platform"]);
        let barrel = string_vec(&["@effect/schema", "effect/sql"]);
        let aliases = string_map(&[("effect", "E"), ("@effect/platform", "Platform")]);

        assert_import_style(
            t,
            Some(&EffectPluginOptions {
                namespace_import_packages: Some(string_vec(&["Effect", " @Effect/Platform ", ""])),
                barrel_import_packages: Some(string_vec(&["@Effect/Schema", " EFFECT/SQL "])),
                import_aliases: Some(string_map(&[
                    ("Effect", "E"),
                    (" @Effect/Platform ", "Platform"),
                    ("", "Invalid"),
                ])),
                top_level_named_reexports: TopLevelNamedReexportsMode("FOLLOW".into()),
                ..Default::default()
            }),
            &namespace,
            &barrel,
            &aliases,
            &TopLevelNamedReexportsMode::Follow,
        );

        assert_import_style(
            t,
            Some(&parse_name_only_plugin(
                t,
                vec![
                    (
                        "namespaceImportPackages",
                        strs(&["  ", "Effect", "\t", " @Effect/Platform "]),
                    ),
                    ("barrelImportPackages", strs(&["", "@Effect/Schema", " EFFECT/SQL "])),
                    (
                        "importAliases",
                        make_plugin_map(vec![
                            ("", s("Ignored")),
                            (" ", s("AlsoIgnored")),
                            ("Effect", s("E")),
                            (" @Effect/Platform ", s("Platform")),
                        ]),
                    ),
                    ("topLevelNamedReexports", s("FOLLOW")),
                ],
            )),
            &namespace,
            &barrel,
            &aliases,
            &TopLevelNamedReexportsMode::Follow,
        );

        let got = parse_name_only_plugin(t, vec![("topLevelNamedReexports", s("IGNORE"))])
            .top_level_named_reexports;
        if got != TopLevelNamedReexportsMode::Ignore {
            t.error(sprintf!(
                "TopLevelNamedReexports = %q, want %q (case-insensitive)",
                got.0.as_ref(),
                TopLevelNamedReexportsMode::Ignore.0.as_ref()
            ));
        }
    });
}

// go test: etscore/options_test.go:421 TestParseFromPlugins_BrandedSchemaClasses cce7ddb1586670f4
#[test]
fn test_parse_from_plugins_branded_schema_classes() {
    ets_go::testing::run(|t| {
        let resolve = |pairs: Vec<(&str, Any)>| -> Option<ResolvedEffectPluginOptions> {
            t.helper();
            ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
                Some(&parse_name_only_plugin(t, pairs)),
                "/repo/src/example.ts",
                "/repo/tsconfig.json",
                true,
            )
        };

        let default = resolve(vec![]);
        let got = ResolvedEffectPluginOptions::get_branded_schema_classes(default.as_ref());
        if got != DEFAULT_BRANDED_SCHEMA_CLASSES.as_slice() {
            t.error(sprintf!(
                "default GetBrandedSchemaClasses() = %v, want %v",
                format!("{got:?}"),
                format!("{:?}", *DEFAULT_BRANDED_SCHEMA_CLASSES)
            ));
        }

        let reduced = resolve(vec![("brandedSchemaClasses", strs(&["Class", "Opaque"]))]);
        let includes = |kind| {
            ResolvedEffectPluginOptions::branded_schema_classes_includes(reduced.as_ref(), kind)
        };
        if !includes("Class") || !includes("Opaque") || includes("TaggedError") {
            t.error(sprintf!(
                "reduced GetBrandedSchemaClasses() = %v, want [Class Opaque]",
                format!(
                    "{:?}",
                    ResolvedEffectPluginOptions::get_branded_schema_classes(reduced.as_ref())
                )
            ));
        }

        let empty = resolve(vec![("brandedSchemaClasses", Any::Slice(vec![]))]);
        if empty.as_ref().is_none_or(|e| e.branded_schema_classes != Some(vec![])) {
            t.error(sprintf!(
                "empty GetBrandedSchemaClasses() = %#v, want empty non-default list",
                format!("{empty:?}")
            ));
        }

        let overridden = resolve(vec![
            ("brandedSchemaClasses", Any::Slice(vec![])),
            (
                "overrides",
                Any::Slice(vec![make_plugin_map(vec![
                    ("include", strs(&["src/**/*.ts"])),
                    (
                        "options",
                        make_plugin_map(vec![("brandedSchemaClasses", strs(&["TaggedClass"]))]),
                    ),
                ])]),
            ),
        ]);
        let got = ResolvedEffectPluginOptions::get_branded_schema_classes(overridden.as_ref());
        if got != string_vec(&["TaggedClass"]).as_slice() {
            t.error(sprintf!(
                "overridden GetBrandedSchemaClasses() = %v, want [TaggedClass]",
                format!("{got:?}")
            ));
        }
    });
}
