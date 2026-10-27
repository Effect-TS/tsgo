use std::borrow::Cow;

use rustc_hash::FxHashMap;

use crate::*;

/// Rust-only: Go's `any` as the parser below inspects it, with one method per type assertion it makes.
///
/// Go's parser receives the tsconfig value tsoptions decoded (`[]any`, `*collections.OrderedMap[string, any]`,
/// `float64`, `string`, `bool`, `nil`) or, from tests, a `map[string]any`. tsrs holds that value as
/// `tsoptions::CompilerOptionsValue`, but this crate cannot depend on tsrs: tsrs's `CompilerOptions` holds
/// `EffectPluginOptions` (patch 007), so tsrs depends on this crate, as Go's `core` imports `etscore`. The tsrs
/// side implements this trait for its value type; tests implement it for their own.
pub trait AnyValue: Sized {
    /// `value == nil`.
    fn is_nil(&self) -> bool;
    /// `value.(bool)`.
    fn as_bool(&self) -> Option<bool>;
    /// `value.(float64)` (a Go `int` is not one).
    fn as_float64(&self) -> Option<f64>;
    /// `value.(string)`.
    fn as_string(&self) -> Option<&str>;
    /// `value.([]any)` (a nil `[]any` is an empty slice).
    fn as_slice(&self) -> Option<&[Self]>;
    /// `value` is a `*collections.OrderedMap[string, any]` or a `map[string]any`: its entries in iteration order
    /// (insertion order for the ordered map, any order for a Go map); `None` for any other value.
    fn as_string_any_map(&self) -> Option<Vec<(&str, &Self)>>;
}

// go: etscore/options_parser.go:12 ParseFromPlugins 35702843286ae20f
// ParseFromPlugins parses the @effect/language-service plugin config from the plugins array.
// The plugins parameter is the value of compilerOptions.plugins from tsconfig.json.
// Returns nil if the plugin is not configured or if diagnosticSeverity is explicitly set to null.
pub fn parse_from_plugins<V: AnyValue>(value: &V) -> Option<EffectPluginOptions> {
    let plugins = value.as_slice()?;

    for p in plugins {
        let Some(get_plugin_value) = as_string_any_map(p) else {
            continue;
        };

        let name = get_plugin_value("name");
        if name.and_then(AnyValue::as_string) != Some(EFFECT_PLUGIN_NAME) {
            continue;
        }

        // Found our plugin, parse the config
        let mut result = EffectPluginOptions {
            refactors: true,                                  // default: true
            diagnostics: true,                                // default: true
            include_suggestions_in_tsc: true,                 // default: true
            quickinfo: true,                                  // default: true
            completions: true,                                // default: true
            goto: true,                                       // default: true
            renames: true,                                    // default: true
            ignore_effect_suggestions_in_tsc_exit_code: true, // default: true
            ignore_effect_warnings_in_tsc_exit_code: false,   // default: false
            diagnostic_severity: Some(FxHashMap::default()),
            ..Default::default()
        };

        // Parse refactors (default: true)
        if let Some(val) = get_plugin_value("refactors") {
            if let Some(b) = val.as_bool() {
                result.refactors = b;
            }
        }

        // Parse diagnostics (default: true)
        if let Some(val) = get_plugin_value("diagnostics") {
            if let Some(b) = val.as_bool() {
                result.diagnostics = b;
            }
        }

        // Parse diagnosticSeverity
        if let Some(diag) = get_plugin_value("diagnosticSeverity") {
            if diag.is_nil() {
                // diagnosticSeverity: null means explicitly disabled
                return None;
            }
            result.diagnostic_severity = Some(parse_diagnostic_severity_map(diag));
        }

        // Parse overrides (default: nil)
        if let Some(val) = get_plugin_value("overrides") {
            if let Some(overrides) = parse_overrides(val) {
                result.overrides = Some(overrides);
            }
        }

        // Parse includeSuggestionsInTsc (default: true)
        if let Some(val) = get_plugin_value("includeSuggestionsInTsc") {
            if let Some(b) = val.as_bool() {
                result.include_suggestions_in_tsc = b;
            }
        }

        // Parse quickinfo (default: true)
        if let Some(val) = get_plugin_value("quickinfo") {
            if let Some(b) = val.as_bool() {
                result.quickinfo = b;
            }
        }

        // Parse completions (default: true)
        if let Some(val) = get_plugin_value("completions") {
            if let Some(b) = val.as_bool() {
                result.completions = b;
            }
        }

        // Parse goto (default: true)
        if let Some(val) = get_plugin_value("goto") {
            if let Some(b) = val.as_bool() {
                result.goto = b;
            }
        }

        // Parse renames (default: true)
        if let Some(val) = get_plugin_value("renames") {
            if let Some(b) = val.as_bool() {
                result.renames = b;
            }
        }

        // Parse ignoreEffectSuggestionsInTscExitCode (default: true)
        if let Some(val) = get_plugin_value("ignoreEffectSuggestionsInTscExitCode") {
            if let Some(b) = val.as_bool() {
                result.ignore_effect_suggestions_in_tsc_exit_code = b;
            }
        }

        // Parse ignoreEffectWarningsInTscExitCode (default: false)
        if let Some(val) = get_plugin_value("ignoreEffectWarningsInTscExitCode") {
            if let Some(b) = val.as_bool() {
                result.ignore_effect_warnings_in_tsc_exit_code = b;
            }
        }

        // Parse ignoreEffectErrorsInTscExitCode (default: false)
        if let Some(val) = get_plugin_value("ignoreEffectErrorsInTscExitCode") {
            if let Some(b) = val.as_bool() {
                result.ignore_effect_errors_in_tsc_exit_code = b;
            }
        }

        // Parse skipDisabledOptimization (default: false)
        if let Some(val) = get_plugin_value("skipDisabledOptimization") {
            if let Some(b) = val.as_bool() {
                result.skip_disabled_optimization = b;
            }
        }

        // Parse keyPatterns (default: nil, use GetKeyPatterns() for defaults)
        if let Some(val) = get_plugin_value("keyPatterns") {
            if let Some(arr) = val.as_slice() {
                result.key_patterns = parse_key_patterns(arr);
            }
        }

        // Parse extendedKeyDetection (default: false)
        if let Some(val) = get_plugin_value("extendedKeyDetection") {
            if let Some(b) = val.as_bool() {
                result.extended_key_detection = b;
            }
        }

        // Parse pipeableMinArgCount (default: 2, handled by GetPipeableMinArgCount)
        if let Some(val) = get_plugin_value("pipeableMinArgCount") {
            if let Some(f) = val.as_float64() {
                result.pipeable_min_arg_count = go_int_from_float64(f);
            }
        }

        // Parse mermaidProvider (default: "", resolved by GetMermaidBaseURL)
        if let Some(val) = get_plugin_value("mermaidProvider") {
            if let Some(s) = val.as_string() {
                result.mermaid_provider = s.to_string();
            }
        }

        // Parse noExternal (default: false)
        if let Some(val) = get_plugin_value("noExternal") {
            if let Some(b) = val.as_bool() {
                result.no_external = b;
            }
        }

        // Parse layerGraphFollowDepth (default: 0, handled by GetLayerGraphFollowDepth)
        if let Some(val) = get_plugin_value("layerGraphFollowDepth") {
            if let Some(f) = val.as_float64() {
                result.layer_graph_follow_depth = go_int_from_float64(f);
            }
        }

        // Parse inlays (default: false)
        if let Some(val) = get_plugin_value("inlays") {
            if let Some(b) = val.as_bool() {
                result.inlays = b;
            }
        }

        // Parse effectFn (default: nil, GetEffectFn() returns ["span"])
        if let Some(val) = get_plugin_value("effectFn") {
            if let Some(arr) = val.as_slice() {
                let mut variants = Vec::with_capacity(arr.len());
                for item in arr {
                    if let Some(s) = item.as_string() {
                        variants.push(s.to_string());
                    }
                }
                result.effect_fn = Some(variants);
            }
        }

        if let Some(val) = get_plugin_value("allowedUnstableApis") {
            if let Some(apis) = parse_string_array_strict(val) {
                result.allowed_unstable_apis = Some(apis);
            }
        }
        if let Some(val) = get_plugin_value("allowedExperimentalApis") {
            if let Some(apis) = parse_string_array_strict(val) {
                result.allowed_experimental_apis = Some(apis);
            }
        }

        // Parse allowedDuplicatedPackages (default: nil)
        if let Some(val) = get_plugin_value("allowedDuplicatedPackages") {
            if let Some(arr) = val.as_slice() {
                let mut pkgs = Vec::with_capacity(arr.len());
                for item in arr {
                    if let Some(s) = item.as_string() {
                        pkgs.push(s.to_string());
                    }
                }
                result.allowed_duplicated_packages = Some(pkgs);
            }
        }

        // Parse namespaceImportPackages (default: nil, getter resolves to empty list)
        if let Some(val) = get_plugin_value("namespaceImportPackages") {
            if let Some(pkgs) = parse_normalized_string_array_strict(val) {
                result.namespace_import_packages = Some(pkgs);
            }
        }

        // Parse barrelImportPackages (default: nil, getter resolves to empty list)
        if let Some(val) = get_plugin_value("barrelImportPackages") {
            if let Some(pkgs) = parse_normalized_string_array_strict(val) {
                result.barrel_import_packages = Some(pkgs);
            }
        }

        // Parse importAliases (default: nil, getter resolves to empty map)
        if let Some(val) = get_plugin_value("importAliases") {
            if let Some(aliases) = parse_normalized_string_map_strict(val) {
                result.import_aliases = Some(aliases);
            }
        }

        // Parse topLevelNamedReexports (default: "", getter resolves to "ignore")
        if let Some(val) = get_plugin_value("topLevelNamedReexports") {
            if let Some(mode) = parse_top_level_named_reexports(val) {
                result.top_level_named_reexports = mode;
            }
        }

        return Some(result);
    }

    None
}

/// Rust-only: Go's `int(f)` conversion of a `float64`: truncation toward zero.
///
/// Go leaves NaN and out-of-range conversions implementation-defined; on amd64 (`CVTTSD2SI`) they give the
/// minimum `int`, which is what this returns (Rust's `as` would saturate and map NaN to 0). JSON has no NaN, but
/// a tsconfig can hold `1e300`.
fn go_int_from_float64(f: f64) -> i64 {
    if f.is_nan() || f >= 9_223_372_036_854_775_808.0 || f < -9_223_372_036_854_775_808.0 {
        return i64::MIN;
    }
    f as i64
}

// go: etscore/options_parser.go:264 parseTopLevelNamedReexports b40c293655847cbf
pub(crate) fn parse_top_level_named_reexports<V: AnyValue>(
    value: &V,
) -> Option<TopLevelNamedReexportsMode> {
    let s = value.as_string()?;
    let mode = TopLevelNamedReexportsMode(Cow::Owned(ets_go::strings::to_lower(s)));
    if mode == TopLevelNamedReexportsMode::Ignore || mode == TopLevelNamedReexportsMode::Follow {
        return Some(mode);
    }
    None
}

// go: etscore/options_parser.go:276 parseNormalizedStringArrayStrict d041452ce54a2a77
pub(crate) fn parse_normalized_string_array_strict<V: AnyValue>(value: &V) -> Option<Vec<String>> {
    let arr = value.as_slice()?;
    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_string()?;
        let normalized = normalize_package_name(s);
        if !normalized.is_empty() {
            result.push(normalized);
        }
    }
    Some(result)
}

// go: etscore/options_parser.go:294 parseNormalizedStringMapStrict d6a5c7eee50029a3
// Rust: Go's two map cases (`*collections.OrderedMap[string, any]`, `map[string]any`) iterate the same way here
// (`AnyValue::as_string_any_map`).
pub(crate) fn parse_normalized_string_map_strict<V: AnyValue>(
    value: &V,
) -> Option<FxHashMap<String, String>> {
    let mut result = FxHashMap::default();
    let alias_map = value.as_string_any_map()?;
    for (k, v) in alias_map {
        let s = v.as_string()?;
        let normalized = normalize_package_name(k);
        if !normalized.is_empty() {
            result.insert(normalized, s.to_string());
        }
    }
    Some(result)
}

// go: etscore/options_parser.go:323 parseOverrides ca04710071effd3f
pub(crate) fn parse_overrides<V: AnyValue>(value: &V) -> Option<Vec<Override>> {
    let arr = value.as_slice()?;

    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(scope_map) = as_string_any_map(item) else {
            continue;
        };

        let mut override_ = Override::default();
        if let Some(include) = scope_map("include") {
            override_.include = parse_string_array_lossy(include);
        }
        if let Some(exclude) = scope_map("exclude") {
            override_.exclude = parse_string_array_lossy(exclude);
        }
        if let Some(options) = scope_map("options") {
            override_.options = parse_override_options(options);
        }

        result.push(override_);
    }

    Some(result)
}

// go: etscore/options_parser.go:353 parseOverrideOptions 5135ef96cc815d73
pub(crate) fn parse_override_options<V: AnyValue>(value: &V) -> OverrideOptions {
    let mut result = OverrideOptions::default();
    let Some(options_map) = as_string_any_map(value) else {
        return result;
    };

    if let Some(value) = options_map("diagnosticSeverity") {
        result.diagnostic_severity = Some(parse_diagnostic_severity_map(value));
    }

    if let Some(value) = options_map("pipeableMinArgCount") {
        if let Some(f) = value.as_float64() {
            let parsed = go_int_from_float64(f);
            result.pipeable_min_arg_count = Some(parsed);
        }
    }
    if let Some(value) = options_map("keyPatterns") {
        if let Some(arr) = value.as_slice() {
            let parsed = parse_key_patterns(arr);
            // Rust: Go stores a pointer to the (possibly nil) result; see the note in options.rs.
            result.key_patterns = Some(parsed.unwrap_or_default());
        }
    }
    if let Some(value) = options_map("extendedKeyDetection") {
        if let Some(b) = value.as_bool() {
            result.extended_key_detection = Some(b);
        }
    }
    if let Some(value) = options_map("allowedUnstableApis") {
        if let Some(apis) = parse_string_array_strict(value) {
            result.allowed_unstable_apis = Some(apis);
        }
    }
    if let Some(value) = options_map("allowedExperimentalApis") {
        if let Some(apis) = parse_string_array_strict(value) {
            result.allowed_experimental_apis = Some(apis);
        }
    }
    if let Some(value) = options_map("allowedDuplicatedPackages") {
        if let Some(pkgs) = parse_string_array_strict(value) {
            result.allowed_duplicated_packages = Some(pkgs);
        }
    }
    if let Some(value) = options_map("effectFn") {
        if let Some(variants) = parse_string_array_strict(value) {
            result.effect_fn = Some(variants);
        }
    }
    if let Some(value) = options_map("namespaceImportPackages") {
        if let Some(pkgs) = parse_normalized_string_array_strict(value) {
            result.namespace_import_packages = Some(pkgs);
        }
    }
    if let Some(value) = options_map("barrelImportPackages") {
        if let Some(pkgs) = parse_normalized_string_array_strict(value) {
            result.barrel_import_packages = Some(pkgs);
        }
    }
    if let Some(value) = options_map("importAliases") {
        if let Some(aliases) = parse_normalized_string_map_strict(value) {
            result.import_aliases = Some(aliases);
        }
    }
    if let Some(value) = options_map("topLevelNamedReexports") {
        if let Some(mode) = parse_top_level_named_reexports(value) {
            result.top_level_named_reexports = Some(mode);
        }
    }

    result
}

// go: etscore/options_parser.go:425 parseStringArrayStrict b8422baac38eb927
pub(crate) fn parse_string_array_strict<V: AnyValue>(value: &V) -> Option<Vec<String>> {
    let arr = value.as_slice()?;
    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_string()?;
        result.push(s.to_string());
    }
    Some(result)
}

// go: etscore/options_parser.go:441 parseStringArrayLossy b5c4a69aed313fe3
pub(crate) fn parse_string_array_lossy<V: AnyValue>(value: &V) -> Option<Vec<String>> {
    let arr = value.as_slice()?;
    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        if let Some(s) = item.as_string() {
            result.push(s.to_string());
        }
    }
    Some(result)
}

// go: etscore/options_parser.go:455 asStringAnyMap ca3f30cd2c75356f
// Rust: the getter answers Go's `(val, exists)` as `Option<&V>`.
pub(crate) fn as_string_any_map<'a, V: AnyValue>(
    value: &'a V,
) -> Option<impl Fn(&str) -> Option<&'a V> + 'a> {
    let m = value.as_string_any_map()?;
    Some(move |key: &str| -> Option<&'a V> {
        m.iter().find(|(k, _)| *k == key).map(|(_, val)| *val)
    })
}

// go: etscore/options_parser.go:470 parseDiagnosticSeverityMap a19bb20bf6af0aab
// parseDiagnosticSeverityMap converts a plugin rule configuration map to map[string]Severity.
pub(crate) fn parse_diagnostic_severity_map<V: AnyValue>(value: &V) -> FxHashMap<String, Severity> {
    let mut result = FxHashMap::default();
    if let Some(m) = value.as_string_any_map() {
        for (k, v) in m {
            if let Some(s) = v.as_string() {
                result.insert(k.to_string(), parse_severity(s));
            }
        }
    }
    result
}

// go: etscore/options_parser.go:490 parseKeyPatterns e496c3e6205ed4ac
// parseKeyPatterns converts a JSON array of key pattern objects to []KeyPattern.
// Rust: `None` is Go's nil slice (no object in the array).
pub(crate) fn parse_key_patterns<V: AnyValue>(arr: &[V]) -> Option<Vec<KeyPattern>> {
    let mut patterns: Option<Vec<KeyPattern>> = None;
    for item in arr {
        let Some(get_value) = as_string_any_map(item) else {
            continue;
        };

        let mut kp = KeyPattern {
            target: "service".to_string(),
            pattern: "default".to_string(),
            skip_leading_path: Some(vec!["src/".to_string()]),
        };

        if let Some(v) = get_value("target") {
            if let Some(s) = v.as_string() {
                kp.target = s.to_string();
            }
        }

        if let Some(v) = get_value("pattern") {
            if let Some(s) = v.as_string() {
                kp.pattern = s.to_string();
            }
        }

        if let Some(v) = get_value("skipLeadingPath") {
            if let Some(arr) = v.as_slice() {
                let mut paths = Vec::with_capacity(arr.len());
                for p in arr {
                    if let Some(s) = p.as_string() {
                        paths.push(s.to_string());
                    }
                }
                kp.skip_leading_path = Some(paths);
            }
        }

        patterns.get_or_insert_with(Vec::new).push(kp);
    }
    patterns
}
