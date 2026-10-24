// Rust-only: internal/effectconfigraw has no Go tests of its own. These are regression tests for the tsrs ports of
// patches 007 (`CompilerOptions::effect`), 013 (the `plugins` parse and the merge callback) and 025 (the config
// paths passed to it), through tsrs's tsconfig parsing with `effectconfigraw.Register` installed. The cases mirror
// etsoxlintrunner/config_test.go `TestEffectFnOptionsWithExtends` (not ported: oxlint is Go-only),
// internal/effectconfigcheck/validation_test.go `TestEffectPluginInheritance` and
// `TestMultipleExtendsAndExistingOptions` (the Effect part), and internal/effecttest/feature_flags_test.go
// `TestEffectOverridesPreserveExtendedConfigBasePaths` (the merged override paths).

use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_shim::core::CompilerOptions;
use ets_shim::core::tspath;
use ets_shim::tsoptions::{
    ParsedCommandLine, new_tsconfig_source_file_from_file_path,
    parse_json_source_file_config_file_content, tsoptionstest,
};
use rustc_hash::FxHashMap;

/// Rust-only: Go's test packages import etscheckerhooks (or etsoxlintrunner), whose `init` calls `Register`.
fn parse(files: &[(&str, &str)], existing: Option<&CompilerOptions>) -> ParsedCommandLine {
    ets_effectconfigraw::register();
    let mut all_files = vec![("/main.ts", "export {}")];
    all_files.extend_from_slice(files);
    let host = tsoptionstest::new_vfs_parse_config_host(&all_files, "/", true);
    let config =
        files.iter().find(|(name, _)| *name == "/tsconfig.json").expect("missing config").1;
    let source = new_tsconfig_source_file_from_file_path(
        "/tsconfig.json",
        tspath::to_path("/tsconfig.json", "/", true),
        config,
    );
    parse_json_source_file_config_file_content(
        source,
        host,
        "/",
        existing,
        None,
        "/tsconfig.json",
        &[],
        None,
    )
}

fn effect(parsed: &ParsedCommandLine) -> Option<Arc<EffectPluginOptions>> {
    parsed.compiler_options()?.effect.get()
}

fn config(options: &str, extra: &str) -> String {
    format!(
        r#"{{"files":["/main.ts"],"compilerOptions":{{"plugins":[{{"name":"@effect/language-service",{options}}}]}}{extra}}}"#
    )
}

#[test]
fn test_effect_fn_options_with_extends() {
    let options = r#"{"plugins":[{"name":"@effect/language-service","effectFn":["span","inferred-span","suggested-span"]}]}"#;
    for (name, config, base) in [
        (
            "inline",
            format!(r#"{{"compilerOptions":{options},"files":["main.ts"]}}"#),
            "{}".to_string(),
        ),
        (
            "inherited",
            r#"{"extends":"./base.json","files":["main.ts"]}"#.to_string(),
            format!(r#"{{"compilerOptions":{options}}}"#),
        ),
        (
            "local",
            format!(
                r#"{{"extends":"./base.json","compilerOptions":{options},"files":["main.ts"]}}"#
            ),
            "{}".to_string(),
        ),
    ] {
        let parsed = parse(&[("/base.json", &base), ("/tsconfig.json", &config)], None);
        assert!(
            parsed.errors.is_empty(),
            "{name}: unexpected config errors: {}",
            parsed.errors.len()
        );
        let Some(effect) = effect(&parsed) else {
            panic!("{name}: Effect plugin options were lost");
        };
        assert_eq!(
            effect.effect_fn.as_deref(),
            Some(
                &["span".to_string(), "inferred-span".to_string(), "suggested-span".to_string()][..]
            ),
            "{name}: unexpected effectFn options"
        );
    }
}

#[test]
fn test_effect_plugin_inheritance() {
    let base = config(r#""diagnostics":false"#, "");
    for (name, files, active) in [
        (
            "omitted plugins inherits Effect",
            vec![("/base.json", base.as_str()), ("/tsconfig.json", r#"{"extends":"./base.json"}"#)],
            true,
        ),
        (
            "empty plugins clears Effect",
            vec![
                ("/base.json", base.as_str()),
                ("/tsconfig.json", r#"{"extends":"./base.json","compilerOptions":{"plugins":[]}}"#),
            ],
            false,
        ),
        (
            "replacement plugins clears Effect",
            vec![
                ("/base.json", base.as_str()),
                (
                    "/tsconfig.json",
                    r#"{"extends":"./base.json","compilerOptions":{"plugins":[{"name":"other-plugin"}]}}"#,
                ),
            ],
            false,
        ),
        (
            "multi-hop inheritance preserves explicit clearing list",
            vec![
                ("/base.json", base.as_str()),
                ("/middle.json", r#"{"extends":"./base.json","compilerOptions":{"plugins":[]}}"#),
                ("/tsconfig.json", r#"{"extends":"./middle.json"}"#),
            ],
            false,
        ),
    ] {
        let effect = effect(&parse(&files, None));
        if active {
            let Some(effect) = effect else {
                panic!("{name}: expected Effect plugin to be active");
            };
            assert!(!effect.diagnostics, "{name}: expected inherited diagnostics=false");
            continue;
        }
        assert!(effect.is_none(), "{name}: expected Effect plugin to be inactive, got {effect:?}");
    }
}

#[test]
fn test_multiple_extends_and_existing_options() {
    let base = config(r#""diagnosticSeverity":{"floatingEfect":"error"}"#, "");
    let severity = config(r#""diagnosticSeverity":{"unknownRuleName":"error"}"#, "");
    let files = [
        ("/base.json", base.as_str()),
        ("/severity.json", severity.as_str()),
        ("/tsconfig.json", r#"{"extends":["./base.json","./severity.json"]}"#),
    ];
    let merged = effect(&parse(&files, None)).expect("multiple extends lost the Effect options");
    let mut want = FxHashMap::default();
    want.insert("floatingEfect".to_string(), Severity::Error);
    want.insert("unknownRuleName".to_string(), Severity::Error);
    assert_eq!(merged.diagnostic_severity.as_ref(), Some(&want), "multiple extends failed");

    let mut existing_severity = FxHashMap::default();
    existing_severity.insert("existingTypo".to_string(), Severity::Error);
    let existing = CompilerOptions {
        effect: Some(Arc::new(EffectPluginOptions {
            diagnostics: true,
            diagnostic_severity: Some(existing_severity.clone()),
            ..Default::default()
        }))
        .into(),
        ..Default::default()
    };
    let parsed = effect(&parse(&files, Some(&existing))).expect("existing options were lost");
    assert_eq!(
        parsed.diagnostic_severity.as_ref(),
        Some(&existing_severity),
        "existing options without a raw plugin stanza replace the config's"
    );
    assert!(
        !Arc::ptr_eq(&parsed, &existing.effect.get().unwrap()),
        "the merged options are a clone of the existing ones"
    );
}

#[test]
fn test_effect_overrides_preserve_extended_config_base_paths() {
    let base = r#"{"compilerOptions":{"plugins":[{"name":"@effect/language-service","overrides":[{"include":["shared/**/*.ts"],"options":{"diagnosticSeverity":{"floatingEffect":"off"}}}]}]}}"#;
    let config = r#"{"extends":"./configs/base.json","compilerOptions":{"plugins":[{"name":"@effect/language-service","diagnosticSeverity":{"floatingEffect":"error"}}]}}"#;
    let parsed = parse(&[("/configs/base.json", base), ("/tsconfig.json", config)], None);
    let effect = effect(&parsed).expect("Effect plugin options were lost");
    let overrides = effect.overrides.as_deref().expect("inherited overrides were lost");
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0].include.as_deref(), Some(&["configs/shared/**/*.ts".to_string()][..]));
    assert_eq!(
        effect.diagnostic_severity.as_ref().and_then(|m| m.get("floatingEffect")),
        Some(&Severity::Error)
    );
}

#[test]
fn test_merge_effect_compiler_options_nil() {
    let source = CompilerOptions {
        effect: Some(Arc::new(EffectPluginOptions::default())).into(),
        ..Default::default()
    };
    let mut target = CompilerOptions::default();
    ets_effectconfigraw::merge_effect_compiler_options(None, Some(&source), None, "", "");
    ets_effectconfigraw::merge_effect_compiler_options(Some(&mut target), None, None, "", "");
    assert!(target.effect.is_none());
    ets_effectconfigraw::merge_effect_compiler_options(
        Some(&mut target),
        Some(&source),
        None,
        "",
        "",
    );
    assert_eq!(target.effect, source.effect);
}
