use std::borrow::Cow;

use ets_etscore::{
    EffectPluginOptions, KeyPattern, Override, ResolvedEffectPluginOptions, Severity,
};
use ets_shim::core::tspath;
use ets_shim::vfs::vfsmatch;
use rustc_hash::FxHashMap;

// Rust: Go's `tspath.RootedFilePath` arguments are plain file names (porting/TSRS_API.md: tsrs has no rooted path
// types), so `tspath.RootedDirectoryPathFromAbsolute` drops out.

// go: internal/pluginoptions/resolver.go:13 ResolveEffectPluginOptionsForSourceFile 7d38bd40f9880eee
// ResolveEffectPluginOptionsForSourceFile resolves ordered path-scoped diagnostic overrides for a file.
pub fn resolve_effect_plugin_options_for_source_file(
    config: Option<&EffectPluginOptions>,
    file_name: &str,
    config_file_path: &str,
    use_case_sensitive_file_names: bool,
) -> Option<ResolvedEffectPluginOptions> {
    let config = config?;
    let overrides = config.overrides.as_deref().unwrap_or_default();
    if overrides.is_empty() {
        return clone_options(Some(config));
    }

    let base_path = tspath::get_directory_path(config_file_path);
    let mut effective: Option<ResolvedEffectPluginOptions> = None;

    for override_ in overrides {
        if !matches_override(override_, file_name, &base_path, use_case_sensitive_file_names) {
            continue;
        }
        if effective.is_none() {
            effective = clone_options(Some(config));
        }
        apply_override(effective.as_mut(), override_);
    }

    if effective.is_none() {
        return clone_options(Some(config));
    }
    effective
}

// go: internal/pluginoptions/resolver.go:41 ResolveDiagnosticSeverityForFile 0effa833ac3413da
// ResolveDiagnosticSeverityForFile resolves ordered diagnostic severity overrides for a file.
//
// Rust: Go returns `config.DiagnosticSeverity` itself when there are no overrides (borrowed here) and a fresh map
// otherwise; `None` is Go's nil map.
pub fn resolve_diagnostic_severity_for_file<'a>(
    config: Option<&'a EffectPluginOptions>,
    file_name: &str,
    config_file_path: &str,
    use_case_sensitive_file_names: bool,
) -> Option<Cow<'a, FxHashMap<String, Severity>>> {
    let config = config?;
    let overrides = config.overrides.as_deref().unwrap_or_default();
    if overrides.is_empty() {
        return config.diagnostic_severity.as_ref().map(Cow::Borrowed);
    }

    let base_path = tspath::get_directory_path(config_file_path);
    let mut resolved =
        clone_diagnostic_severity(config.diagnostic_severity.as_ref()).unwrap_or_default();

    for override_ in overrides {
        if !matches_override(override_, file_name, &base_path, use_case_sensitive_file_names) {
            continue;
        }
        let Some(diagnostic_severity) =
            override_.options.diagnostic_severity.as_ref().filter(|severity| !severity.is_empty())
        else {
            continue;
        };
        resolved.extend(diagnostic_severity.iter().map(|(k, v)| (k.clone(), *v)));
    }

    Some(Cow::Owned(resolved))
}

// go: internal/pluginoptions/resolver.go:68 matchesOverride 68e4edc52efeccc9
pub(crate) fn matches_override(
    override_: &Override,
    file_name: &str,
    base_path: &str,
    use_case_sensitive_file_names: bool,
) -> bool {
    let include = override_.include.as_deref().unwrap_or_default();
    if !include.is_empty() {
        let matcher = vfsmatch::new_spec_matcher(
            include,
            base_path,
            vfsmatch::Usage::Files,
            use_case_sensitive_file_names,
        );
        if !matcher.is_some_and(|matcher| matcher.match_string(file_name)) {
            return false;
        }
    }

    let exclude = override_.exclude.as_deref().unwrap_or_default();
    if !exclude.is_empty() {
        let matcher = vfsmatch::new_spec_matcher(
            exclude,
            base_path,
            vfsmatch::Usage::Exclude,
            use_case_sensitive_file_names,
        );
        if matcher.is_some_and(|matcher| matcher.match_string(file_name)) {
            return false;
        }
    }

    true
}

// go: internal/pluginoptions/resolver.go:91 applyOverride 27e032cbc3703576
pub(crate) fn apply_override(
    target: Option<&mut ResolvedEffectPluginOptions>,
    override_: &Override,
) {
    let Some(target) = target else {
        return;
    };

    if let Some(pipeable_min_arg_count) = override_.options.pipeable_min_arg_count {
        target.pipeable_min_arg_count = pipeable_min_arg_count;
    }
    if let Some(key_patterns) = &override_.options.key_patterns {
        target.key_patterns = clone_key_patterns(key_patterns);
    }
    if let Some(extended_key_detection) = override_.options.extended_key_detection {
        target.extended_key_detection = extended_key_detection;
    }
    if let Some(allowed_unstable_apis) = &override_.options.allowed_unstable_apis {
        target.allowed_unstable_apis.clone_from(allowed_unstable_apis);
    }
    if let Some(allowed_experimental_apis) = &override_.options.allowed_experimental_apis {
        target.allowed_experimental_apis.clone_from(allowed_experimental_apis);
    }
    if let Some(allowed_duplicated_packages) = &override_.options.allowed_duplicated_packages {
        target.allowed_duplicated_packages.clone_from(allowed_duplicated_packages);
    }
    if let Some(effect_fn) = &override_.options.effect_fn {
        target.effect_fn.clone_from(effect_fn);
    }
    if let Some(branded_schema_classes) = &override_.options.branded_schema_classes {
        target.branded_schema_classes = Some(branded_schema_classes.clone());
    }
    if let Some(namespace_import_packages) = &override_.options.namespace_import_packages {
        target.namespace_import_packages.clone_from(namespace_import_packages);
    }
    if let Some(barrel_import_packages) = &override_.options.barrel_import_packages {
        target.barrel_import_packages.clone_from(barrel_import_packages);
    }
    if let Some(import_aliases) = &override_.options.import_aliases {
        target.import_aliases = clone_string_map(Some(import_aliases)).unwrap_or_default();
    }
    if let Some(top_level_named_reexports) = &override_.options.top_level_named_reexports {
        target.top_level_named_reexports.clone_from(top_level_named_reexports);
    }
}

// go: internal/pluginoptions/resolver.go:134 cloneOptions 72b2679991789e06
pub(crate) fn clone_options(
    config: Option<&EffectPluginOptions>,
) -> Option<ResolvedEffectPluginOptions> {
    let config = config?;

    let cloned = ResolvedEffectPluginOptions {
        namespace_import_packages: config.namespace_import_packages.clone().unwrap_or_default(),
        barrel_import_packages: config.barrel_import_packages.clone().unwrap_or_default(),
        import_aliases: clone_string_map(config.import_aliases.as_ref()).unwrap_or_default(),
        top_level_named_reexports: config.top_level_named_reexports.clone(),
        key_patterns: clone_key_patterns(config.key_patterns.as_deref().unwrap_or_default()),
        extended_key_detection: config.extended_key_detection,
        pipeable_min_arg_count: config.pipeable_min_arg_count,
        allowed_unstable_apis: config.allowed_unstable_apis.clone().unwrap_or_default(),
        allowed_experimental_apis: config.allowed_experimental_apis.clone().unwrap_or_default(),
        allowed_duplicated_packages: config.allowed_duplicated_packages.clone().unwrap_or_default(),
        effect_fn: config.effect_fn.clone().unwrap_or_default(),
        branded_schema_classes: config.branded_schema_classes.clone(),
    };

    Some(cloned)
}

// go: internal/pluginoptions/resolver.go:157 cloneDiagnosticSeverity ec59d3fe5b86a170
pub(crate) fn clone_diagnostic_severity(
    input: Option<&FxHashMap<String, Severity>>,
) -> Option<FxHashMap<String, Severity>> {
    let input = input?;
    let mut cloned = FxHashMap::with_capacity_and_hasher(input.len(), rustc_hash::FxBuildHasher);
    cloned.extend(input.iter().map(|(k, v)| (k.clone(), *v)));
    Some(cloned)
}

// go: internal/pluginoptions/resolver.go:166 cloneStringMap 1eb570f6c2e7f6d5
pub(crate) fn clone_string_map(
    input: Option<&FxHashMap<String, String>>,
) -> Option<FxHashMap<String, String>> {
    let input = input?;
    let mut cloned = FxHashMap::with_capacity_and_hasher(input.len(), rustc_hash::FxBuildHasher);
    cloned.extend(input.iter().map(|(k, v)| (k.clone(), v.clone())));
    Some(cloned)
}

// go: internal/pluginoptions/resolver.go:175 cloneKeyPatterns 67cd1e32945e9565
pub(crate) fn clone_key_patterns(patterns: &[KeyPattern]) -> Vec<KeyPattern> {
    if patterns.is_empty() {
        return Vec::new();
    }
    let mut cloned = Vec::with_capacity(patterns.len());
    for pattern in patterns {
        cloned.push(KeyPattern {
            target: pattern.target.clone(),
            pattern: pattern.pattern.clone(),
            skip_leading_path: append_nil(pattern.skip_leading_path.as_deref()),
        });
    }
    cloned
}

/// Rust-only: Go's `append([]string(nil), s...)`, which is nil when `s` is empty.
fn append_nil(s: Option<&[String]>) -> Option<Vec<String>> {
    s.filter(|s| !s.is_empty()).map(<[String]>::to_vec)
}
