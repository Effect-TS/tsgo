use ets_etscore::{
    EFFECT_PLUGIN_NAME, EffectPluginOptions, KeyPattern, Override, OverrideOptions, Severity,
    TopLevelNamedReexportsMode,
};
use std::sync::Arc;

use ets_shim::core::CompilerOptions;
use ets_shim::core::collections::OrderedMap;
use ets_shim::core::stringutil::go_strings_to_lower;
use ets_shim::core::tspath;
use ets_shim::tsoptions::{self, CompilerOptionsValue};
use rustc_hash::FxHashMap;

// go: internal/effectconfigraw/hooks.go:16 Register 62833b0e8ff9c311
// Register wires Effect-specific tsconfig merge hooks into TypeScript-Go.
pub fn register() {
    tsoptions::register_merge_compiler_options_callback(Arc::new(
        |target_options: &mut CompilerOptions,
         source_options: &CompilerOptions,
         raw_source: Option<&CompilerOptionsValue>,
         source_config_path: &str,
         base_path: &str| {
            merge_effect_compiler_options(
                Some(target_options),
                Some(source_options),
                raw_source,
                source_config_path,
                base_path,
            );
        },
    ));
}

// go: internal/effectconfigraw/hooks.go:21 MergeEffectCompilerOptions 2ecadb5c2f558b29
// MergeEffectCompilerOptions customizes merging for Effect plugin config.
//
// Rust: `CompilerOptions::effect` is Go's `*etscore.EffectPluginOptions` as an `Option<Arc<_>>` (patch 007); every
// assignment stores a fresh clone, as Go assigns fresh pointers.
pub fn merge_effect_compiler_options(
    target_options: Option<&mut CompilerOptions>,
    source_options: Option<&CompilerOptions>,
    raw_source: Option<&CompilerOptionsValue>,
    source_config_path: &str,
    base_path: &str,
) {
    let (Some(target_options), Some(source_options)) = (target_options, source_options) else {
        return;
    };
    let compiler_options_raw = get_compiler_options_raw(raw_source);
    let Some(mut source_effect) = clone_effect_options(source_options.effect.get().as_deref())
    else {
        if compiler_options_raw.is_some_and(|raw| raw.contains_key("plugins")) {
            target_options.effect.set(None);
        }
        return;
    };
    rewrite_effect_options_overrides(Some(&mut source_effect), source_config_path, base_path);
    let source_plugin_raw = get_effect_plugin_raw(raw_source);
    if source_plugin_raw.is_none() {
        // No local plugin stanza means sourceEffect already represents the fully
        // merged config for this branch. Carry it forward so pass-through extends
        // hops do not drop inherited Effect settings.
        target_options.effect.set(Some(Arc::new(source_effect)));
        return;
    }

    if target_options.effect.is_none() {
        target_options.effect.set(Some(Arc::new(source_effect)));
        return;
    }

    let target_effect = target_options.effect.get();
    target_options.effect.set(
        merge_effect_options(target_effect.as_deref(), Some(&source_effect), source_plugin_raw)
            .map(Arc::new),
    );
}

// go: internal/effectconfigraw/hooks.go:51 mergeEffectOptions 0b845e9fec47e168
pub(crate) fn merge_effect_options(
    target: Option<&EffectPluginOptions>,
    source: Option<&EffectPluginOptions>,
    source_plugin_raw: Option<&OrderedMap<String, CompilerOptionsValue>>,
) -> Option<EffectPluginOptions> {
    let Some(mut merged) = clone_effect_options(target) else {
        return clone_effect_options(source);
    };
    let (Some(source), Some(source_plugin_raw)) = (source, source_plugin_raw) else {
        return Some(merged);
    };

    if source_plugin_raw.contains_key("refactors") {
        merged.refactors = source.refactors;
    }
    if source_plugin_raw.contains_key("diagnostics") {
        merged.diagnostics = source.diagnostics;
    }
    if source_plugin_raw.contains_key("includeSuggestionsInTsc") {
        merged.include_suggestions_in_tsc = source.include_suggestions_in_tsc;
    }
    if source_plugin_raw.contains_key("quickinfo") {
        merged.quickinfo = source.quickinfo;
    }
    if source_plugin_raw.contains_key("completions") {
        merged.completions = source.completions;
    }
    if source_plugin_raw.contains_key("goto") {
        merged.goto = source.goto;
    }
    if source_plugin_raw.contains_key("renames") {
        merged.renames = source.renames;
    }
    if source_plugin_raw.contains_key("ignoreEffectSuggestionsInTscExitCode") {
        merged.ignore_effect_suggestions_in_tsc_exit_code =
            source.ignore_effect_suggestions_in_tsc_exit_code;
    }
    if source_plugin_raw.contains_key("ignoreEffectWarningsInTscExitCode") {
        merged.ignore_effect_warnings_in_tsc_exit_code =
            source.ignore_effect_warnings_in_tsc_exit_code;
    }
    if source_plugin_raw.contains_key("ignoreEffectErrorsInTscExitCode") {
        merged.ignore_effect_errors_in_tsc_exit_code = source.ignore_effect_errors_in_tsc_exit_code;
    }
    if source_plugin_raw.contains_key("skipDisabledOptimization") {
        merged.skip_disabled_optimization = source.skip_disabled_optimization;
    }
    if source_plugin_raw.contains_key("mermaidProvider") {
        merged.mermaid_provider.clone_from(&source.mermaid_provider);
    }
    if source_plugin_raw.contains_key("noExternal") {
        merged.no_external = source.no_external;
    }
    if source_plugin_raw.contains_key("layerGraphFollowDepth") {
        merged.layer_graph_follow_depth = source.layer_graph_follow_depth;
    }
    if source_plugin_raw.contains_key("inlays") {
        merged.inlays = source.inlays;
    }
    if source_plugin_raw.contains_key("namespaceImportPackages") {
        merged.namespace_import_packages = append_nil(source.namespace_import_packages.as_deref());
    }
    if source_plugin_raw.contains_key("barrelImportPackages") {
        merged.barrel_import_packages = append_nil(source.barrel_import_packages.as_deref());
    }
    if source_plugin_raw.contains_key("importAliases") {
        merged.import_aliases = clone_string_map(source.import_aliases.as_ref());
    }
    if source_plugin_raw.contains_key("topLevelNamedReexports") {
        merged.top_level_named_reexports.clone_from(&source.top_level_named_reexports);
    }
    if source_plugin_raw.contains_key("keyPatterns") {
        merged.key_patterns = clone_key_patterns(source.key_patterns.as_deref());
    }
    if source_plugin_raw.contains_key("extendedKeyDetection") {
        merged.extended_key_detection = source.extended_key_detection;
    }
    if source_plugin_raw.contains_key("pipeableMinArgCount") {
        merged.pipeable_min_arg_count = source.pipeable_min_arg_count;
    }
    if source_plugin_raw.contains_key("allowedUnstableApis") {
        merged.allowed_unstable_apis = append_nil(source.allowed_unstable_apis.as_deref());
    }
    if source_plugin_raw.contains_key("allowedExperimentalApis") {
        merged.allowed_experimental_apis = append_nil(source.allowed_experimental_apis.as_deref());
    }
    if source_plugin_raw.contains_key("allowedDuplicatedPackages") {
        merged.allowed_duplicated_packages =
            append_nil(source.allowed_duplicated_packages.as_deref());
    }
    if source_plugin_raw.contains_key("effectFn") {
        merged.effect_fn = append_nil(source.effect_fn.as_deref());
    }
    if source_plugin_raw.contains_key("brandedSchemaClasses") {
        merged.branded_schema_classes.clone_from(&source.branded_schema_classes);
    }
    if source_plugin_raw.contains_key("diagnosticSeverity") {
        merged.diagnostic_severity = merge_severity_maps(
            merged.diagnostic_severity.as_ref(),
            source.diagnostic_severity.as_ref(),
        );
    }
    if source_plugin_raw.contains_key("overrides") {
        let mut overrides = merged.overrides.take().unwrap_or_default();
        overrides.extend(clone_overrides(source.overrides.as_deref()).unwrap_or_default());
        // Go's `append(append([]Override(nil), a...), b...)` is nil when both are empty.
        merged.overrides = if overrides.is_empty() { None } else { Some(overrides) };
    }

    Some(merged)
}

// go: internal/effectconfigraw/hooks.go:151 rewriteEffectOptionsOverrides d6bef51df923dd33
pub(crate) fn rewrite_effect_options_overrides(
    effect: Option<&mut EffectPluginOptions>,
    source_config_path: &str,
    base_path: &str,
) {
    let Some(effect) = effect else {
        return;
    };
    let Some(overrides) = effect.overrides.as_mut().filter(|overrides| !overrides.is_empty())
    else {
        return;
    };
    if source_config_path.is_empty() {
        return;
    }
    let relative_difference = tspath::convert_to_relative_path(
        &tspath::get_directory_path(source_config_path),
        &tspath::ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: base_path.to_string(),
        },
    );
    if relative_difference.is_empty() {
        return;
    }
    for override_ in overrides {
        override_.include = rewrite_specs(override_.include.take(), &relative_difference);
        override_.exclude = rewrite_specs(override_.exclude.take(), &relative_difference);
    }
}

// go: internal/effectconfigraw/hooks.go:165 rewriteSpecs 87585c4268a94615
pub(crate) fn rewrite_specs(
    specs: Option<Vec<String>>,
    relative_difference: &str,
) -> Option<Vec<String>> {
    let specs = specs?;
    if specs.is_empty() {
        return Some(specs);
    }
    if relative_difference.is_empty() {
        return Some(specs);
    }
    let mut rewritten = Vec::with_capacity(specs.len());
    for spec in specs {
        if starts_with_config_dir_template(&spec) || tspath::is_rooted_disk_path(&spec) {
            rewritten.push(spec);
            continue;
        }
        rewritten.push(tspath::combine_paths(relative_difference, &[&spec]));
    }
    Some(rewritten)
}

// go: internal/effectconfigraw/hooks.go:180 getEffectPluginRaw 94f81c98a2098c2d
pub(crate) fn get_effect_plugin_raw(
    raw: Option<&CompilerOptionsValue>,
) -> Option<&OrderedMap<String, CompilerOptionsValue>> {
    let compiler_options_raw = get_compiler_options_raw(raw)?;
    let plugins: &[CompilerOptionsValue] = match compiler_options_raw.get("plugins") {
        Some(CompilerOptionsValue::Array(plugins)) => plugins,
        _ => &[],
    };
    let (plugin, _) = find_effect_plugin_raw(plugins);
    plugin
}

// go: internal/effectconfigraw/hooks.go:190 getCompilerOptionsRaw b1674208a866c134
pub(crate) fn get_compiler_options_raw(
    raw: Option<&CompilerOptionsValue>,
) -> Option<&OrderedMap<String, CompilerOptionsValue>> {
    if let Some(CompilerOptionsValue::Object(raw_map)) = raw {
        if let Some(CompilerOptionsValue::Object(compiler_options_raw)) =
            raw_map.get("compilerOptions")
        {
            return Some(compiler_options_raw);
        }
    }
    None
}

// go: internal/effectconfigraw/hooks.go:199 findEffectPluginRaw f4a7d986e379255a
pub(crate) fn find_effect_plugin_raw(
    plugins: &[CompilerOptionsValue],
) -> (Option<&OrderedMap<String, CompilerOptionsValue>>, i64) {
    for (i, plugin_value) in plugins.iter().enumerate() {
        let CompilerOptionsValue::Object(plugin) = plugin_value else {
            continue;
        };
        if let Some(CompilerOptionsValue::String(name)) = plugin.get("name")
            && name == EFFECT_PLUGIN_NAME
        {
            return (Some(plugin), i as i64);
        }
    }
    (None, -1)
}

// go: internal/effectconfigraw/hooks.go:212 cloneEffectOptions cd4e42a245065d27
pub(crate) fn clone_effect_options(
    source: Option<&EffectPluginOptions>,
) -> Option<EffectPluginOptions> {
    let source = source?;
    let mut cloned = source.clone();
    cloned.key_patterns = clone_key_patterns(source.key_patterns.as_deref());
    cloned.allowed_unstable_apis = append_nil(source.allowed_unstable_apis.as_deref());
    cloned.allowed_experimental_apis = append_nil(source.allowed_experimental_apis.as_deref());
    cloned.allowed_duplicated_packages = append_nil(source.allowed_duplicated_packages.as_deref());
    cloned.effect_fn = append_nil(source.effect_fn.as_deref());
    cloned.branded_schema_classes.clone_from(&source.branded_schema_classes);
    cloned.diagnostic_severity = merge_severity_maps(None, source.diagnostic_severity.as_ref());
    cloned.overrides = clone_overrides(source.overrides.as_deref());
    cloned.namespace_import_packages = append_nil(source.namespace_import_packages.as_deref());
    cloned.barrel_import_packages = append_nil(source.barrel_import_packages.as_deref());
    cloned.import_aliases = clone_string_map(source.import_aliases.as_ref());
    Some(cloned)
}

// go: internal/effectconfigraw/hooks.go:231 cloneOverrides c608f33d334e8daa
pub(crate) fn clone_overrides(source: Option<&[Override]>) -> Option<Vec<Override>> {
    let source = source.filter(|source| !source.is_empty())?;
    let mut cloned = Vec::with_capacity(source.len());
    for override_ in source {
        cloned.push(Override {
            include: append_nil(override_.include.as_deref()),
            exclude: append_nil(override_.exclude.as_deref()),
            options: OverrideOptions {
                diagnostic_severity: merge_severity_maps(
                    None,
                    override_.options.diagnostic_severity.as_ref(),
                ),
                pipeable_min_arg_count: clone_int_ptr(override_.options.pipeable_min_arg_count),
                key_patterns: clone_key_patterns_ptr(override_.options.key_patterns.as_deref()),
                extended_key_detection: clone_bool_ptr(override_.options.extended_key_detection),
                allowed_unstable_apis: clone_string_slice_ptr(
                    override_.options.allowed_unstable_apis.as_deref(),
                ),
                allowed_experimental_apis: clone_string_slice_ptr(
                    override_.options.allowed_experimental_apis.as_deref(),
                ),
                allowed_duplicated_packages: clone_string_slice_ptr(
                    override_.options.allowed_duplicated_packages.as_deref(),
                ),
                effect_fn: clone_string_slice_ptr(override_.options.effect_fn.as_deref()),
                branded_schema_classes: clone_string_slice_ptr_keep_empty(
                    override_.options.branded_schema_classes.as_deref(),
                ),
                namespace_import_packages: clone_string_slice_ptr(
                    override_.options.namespace_import_packages.as_deref(),
                ),
                barrel_import_packages: clone_string_slice_ptr(
                    override_.options.barrel_import_packages.as_deref(),
                ),
                import_aliases: clone_string_map_ptr(override_.options.import_aliases.as_ref()),
                top_level_named_reexports: clone_top_level_named_reexports_ptr(
                    override_.options.top_level_named_reexports.as_ref(),
                ),
            },
        });
    }
    Some(cloned)
}

// go: internal/effectconfigraw/hooks.go:260 cloneKeyPatterns 466e178d851647a9
pub(crate) fn clone_key_patterns(source: Option<&[KeyPattern]>) -> Option<Vec<KeyPattern>> {
    let source = source.filter(|source| !source.is_empty())?;
    let mut cloned = Vec::with_capacity(source.len());
    for pattern in source {
        cloned.push(KeyPattern {
            target: pattern.target.clone(),
            pattern: pattern.pattern.clone(),
            skip_leading_path: append_nil(pattern.skip_leading_path.as_deref()),
        });
    }
    Some(cloned)
}

// go: internal/effectconfigraw/hooks.go:275 cloneKeyPatternsPtr e1720592740d7a60
// Rust: a `*[]KeyPattern` is an `Option<Vec<KeyPattern>>` (see etscore's options.rs), so the pointee's nil
// collapses to empty.
pub(crate) fn clone_key_patterns_ptr(source: Option<&[KeyPattern]>) -> Option<Vec<KeyPattern>> {
    let source = source?;
    let cloned = clone_key_patterns(Some(source));
    Some(cloned.unwrap_or_default())
}

// go: internal/effectconfigraw/hooks.go:283 cloneStringMap a771cb728713a7ff
pub(crate) fn clone_string_map(
    source: Option<&FxHashMap<String, String>>,
) -> Option<FxHashMap<String, String>> {
    let source = source.filter(|source| !source.is_empty())?;
    let mut cloned = FxHashMap::with_capacity_and_hasher(source.len(), rustc_hash::FxBuildHasher);
    cloned.extend(source.iter().map(|(k, v)| (k.clone(), v.clone())));
    Some(cloned)
}

// go: internal/effectconfigraw/hooks.go:292 mergeSeverityMaps 78b0a914e319f9b0
pub(crate) fn merge_severity_maps(
    target: Option<&FxHashMap<String, Severity>>,
    source: Option<&FxHashMap<String, Severity>>,
) -> Option<FxHashMap<String, Severity>> {
    let Some(source) = source else {
        let target = target?;
        let mut cloned =
            FxHashMap::with_capacity_and_hasher(target.len(), rustc_hash::FxBuildHasher);
        cloned.extend(target.iter().map(|(k, v)| (k.clone(), *v)));
        return Some(cloned);
    };
    let target_len = target.map_or(0, FxHashMap::len);
    let mut merged =
        FxHashMap::with_capacity_and_hasher(target_len + source.len(), rustc_hash::FxBuildHasher);
    if let Some(target) = target {
        merged.extend(target.iter().map(|(k, v)| (k.clone(), *v)));
    }
    merged.extend(source.iter().map(|(k, v)| (k.clone(), *v)));
    Some(merged)
}

// go: internal/effectconfigraw/hooks.go:307 cloneIntPtr ada8d075674754b4
pub(crate) fn clone_int_ptr(source: Option<i64>) -> Option<i64> {
    let v = source?;
    Some(v)
}

// go: internal/effectconfigraw/hooks.go:315 cloneBoolPtr ad5cfe99573d4c2d
pub(crate) fn clone_bool_ptr(source: Option<bool>) -> Option<bool> {
    let v = source?;
    Some(v)
}

// go: internal/effectconfigraw/hooks.go:323 cloneStringMapPtr d18c69bb4cb32388
// Rust: `maps.Clone` keeps nil-ness, which a `*map` field cannot show here (see etscore's options.rs).
pub(crate) fn clone_string_map_ptr(
    source: Option<&FxHashMap<String, String>>,
) -> Option<FxHashMap<String, String>> {
    let source = source?;
    let v = source.clone();
    Some(v)
}

// go: internal/effectconfigraw/hooks.go:331 cloneTopLevelNamedReexportsPtr 4c0c2c6ce8193713
pub(crate) fn clone_top_level_named_reexports_ptr(
    source: Option<&TopLevelNamedReexportsMode>,
) -> Option<TopLevelNamedReexportsMode> {
    let source = source?;
    let v = source.clone();
    Some(v)
}

// go: internal/effectconfigraw/hooks.go:339 cloneStringSlicePtr 35368310ccb9faf6
// Rust: the pointee's `append([]string(nil), ...)` nil collapses to empty (see etscore's options.rs).
pub(crate) fn clone_string_slice_ptr(source: Option<&[String]>) -> Option<Vec<String>> {
    let source = source?;
    let v = source.to_vec();
    Some(v)
}

// go: internal/effectconfigraw/hooks.go:349 cloneStringSlicePtrKeepEmpty 4b75add232a78ed6
// cloneStringSlicePtrKeepEmpty is like cloneStringSlicePtr but keeps an empty list non-nil,
// for options where [] differs from unset.
pub(crate) fn clone_string_slice_ptr_keep_empty(source: Option<&[String]>) -> Option<Vec<String>> {
    let source = source?;
    let v = source.to_vec();
    Some(v)
}

// go: internal/effectconfigraw/hooks.go:357 configDirTemplate 0d5562b4a971c61f
pub(crate) const CONFIG_DIR_TEMPLATE: &str = "${configDir}";

// go: internal/effectconfigraw/hooks.go:359 startsWithConfigDirTemplate b932a4ec464d99c0
// Rust: Go takes `any` and returns false for a non-string; every caller passes a string.
pub(crate) fn starts_with_config_dir_template(value: &str) -> bool {
    go_strings_to_lower(value).starts_with(&go_strings_to_lower(CONFIG_DIR_TEMPLATE))
}

/// Rust-only: Go's `append([]string(nil), s...)` of a slice field, which is nil when `s` is empty.
fn append_nil(s: Option<&[String]>) -> Option<Vec<String>> {
    s.filter(|s| !s.is_empty()).map(<[String]>::to_vec)
}
