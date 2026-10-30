use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::tspath;
use ets_shim::core::{CompilerOptions, Tristate};
use ets_shim::diagnostics::{self, Category};
use ets_shim::tsoptions::{
    ExtendedConfigCache, ParseConfigHost, ParsedCommandLine,
    new_tsconfig_source_file_from_file_path, parse_json_source_file_config_file_content,
};
use ets_shim::vfs::{FS, bundled, vfstest};
use rustc_hash::FxHashMap;

// go test: internal/effectconfigcheck/validation_test.go:22 parseHost 140325da404d2758
struct ParseHost {
    fs: Arc<dyn FS>,
}

impl ParseConfigHost for ParseHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }
    // Rust: Go's `tsoptions.NewDefaultParseConfigHost` has no current directory; the tests pass absolute paths.
    fn get_current_directory(&self) -> &'static str {
        ""
    }
}

// go test: internal/effectconfigcheck/validation_test.go:24 newHost 491665c7ddc49098
// Rust: the host is leaked (tsrs takes `&'static dyn ParseConfigHost`; porting/TSRS_API.md, "Gaps"). The test package
// imports etscheckerhooks, whose `init` registers the callbacks: `ets_etscheckerhooks::init()` here.
fn new_host(files: &[(&str, &str)]) -> &'static ParseHost {
    ets_etscheckerhooks::init();
    let mut entries: Vec<(String, String)> =
        vec![("/main.ts".to_string(), "export {}".to_string())];
    for &(name, text) in files {
        entries.push((name.to_string(), text.to_string()));
    }
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(entries, true)));
    Box::leak(Box::new(ParseHost { fs }))
}

// go test: internal/effectconfigcheck/validation_test.go:32 parse 0125bcab49ba9706
fn parse(
    host: &'static ParseHost,
    path: &str,
    cache: Option<&dyn ExtendedConfigCache>,
) -> ParsedCommandLine {
    let Some(text) = host.fs.read_file(path) else {
        panic!("missing config {path}");
    };
    let source =
        new_tsconfig_source_file_from_file_path(path, tspath::to_path(path, "", true), &text);
    parse_json_source_file_config_file_content(
        source,
        host,
        &tspath::get_directory_path(path),
        None,
        None,
        path,
        &[],
        cache,
    )
}

// go test: internal/effectconfigcheck/validation_test.go:42 config f9526e3d9bec24d3
fn config(options: &str, extra: &str) -> String {
    format!(
        r#"{{"files":["/main.ts"],"compilerOptions":{{"plugins":[{{"name":"@effect/language-service",{options}}}]}}{extra}}}"#
    )
}

// go test: internal/effectconfigcheck/validation_test.go:46 unknownDiagnostics b5d9d8dd701e7ed1
fn unknown_diagnostics(config: &ParsedCommandLine) -> Vec<P<Diagnostic>> {
    diagnostics_with_code(
        config,
        diagnostics::UNKNOWN_EFFECT_DIAGNOSTIC_RULE_0_IN_DIAGNOSTIC_SEVERITY_EFFECT_UNKNOWN_RULE_NAME.code(),
    )
}

// go test: internal/effectconfigcheck/validation_test.go:50 deprecatedDiagnostics dfc461d82e0f14f4
fn deprecated_diagnostics(config: &ParsedCommandLine) -> Vec<P<Diagnostic>> {
    diagnostics_with_code(
        config,
        diagnostics::EFFECT_DIAGNOSTIC_RULE_0_IS_DEPRECATED_USE_1_INSTEAD_EFFECT_DEPRECATED_RULE_NAME.code(),
    )
}

// go test: internal/effectconfigcheck/validation_test.go:54 diagnosticsWithCode 8ce5e98b272c08f4
fn diagnostics_with_code(config: &ParsedCommandLine, code: i32) -> Vec<P<Diagnostic>> {
    let mut result = Vec::new();
    for &diagnostic in &config.errors {
        if diagnostic.code() == code {
            result.push(diagnostic);
        }
    }
    result
}

/// Rust-only: Go's `diagnostic.File().Text()[diagnostic.Pos():diagnostic.End()]`.
fn underline(diagnostic: P<Diagnostic>) -> &'static str {
    let file = diagnostic.file().expect("located diagnostic");
    &file.text()[diagnostic.pos().unwrap() as usize..diagnostic.end().unwrap() as usize]
}

/// Rust-only: Go's `&tsc.ExtendedConfigCache{}`.
fn extended_config_cache() -> ets_shim::execute::tsc::ExtendedConfigCache {
    ets_shim::execute::tsc::ExtendedConfigCache::default()
}

// go test: internal/effectconfigcheck/validation_test.go:64 TestUnknownRuleNames 452911343bceea8e
#[test]
fn test_unknown_rule_names() {
    for (name, options, count, category) in [
        ("typo", r#""diagnosticSeverity":{"floatingEfect":"error"}"#, 1, Category::Warning),
        (
            "unknown disabled rule still invalid",
            r#""diagnosticSeverity":{"floatingEfect":"off"}"#,
            1,
            Category::Warning,
        ),
        (
            "known names",
            r#""diagnosticSeverity":{"floatingEffect":"error","unusedDirective":"warning","unknownRuleName":"warning","deprecatedRuleName":"warning","preferSchemaOverJson":"warning"}"#,
            0,
            Category::Warning,
        ),
        (
            "error",
            r#""diagnosticSeverity":{"floatingEfect":"error","unknownRuleName":"error"}"#,
            1,
            Category::Error,
        ),
        (
            "suggestion",
            r#""diagnosticSeverity":{"floatingEfect":"error","unknownRuleName":"suggestion"}"#,
            1,
            Category::Suggestion,
        ),
        (
            "off",
            r#""diagnosticSeverity":{"floatingEfect":"error","unknownRuleName":"off"}"#,
            0,
            Category::Warning,
        ),
        (
            "deprecatedRuleName off",
            r#""diagnosticSeverity":{"floatingEfect":"error","deprecatedRuleName":"off"}"#,
            1,
            Category::Warning,
        ),
        (
            "disabled",
            r#""diagnostics":false,"diagnosticSeverity":{"floatingEfect":"error"}"#,
            0,
            Category::Warning,
        ),
        (
            "null",
            r#""diagnosticSeverity":null,"overrides":[{"options":{"diagnosticSeverity":{"floatingEfect":"error"}}}]"#,
            0,
            Category::Warning,
        ),
        (
            "override",
            r#""overrides":[null,{"options":{"diagnosticSeverity":{"floatingEfect":"error"}}}]"#,
            1,
            Category::Warning,
        ),
    ] {
        let host = new_host(&[("/tsconfig.json", &config(options, ""))]);
        let parsed = parse(host, "/tsconfig.json", None);
        let got = unknown_diagnostics(&parsed);
        assert_eq!(
            got.len(),
            count,
            "{name}: want {count} warnings, got {}: {:?}",
            got.len(),
            parsed.errors.iter().map(|d| d.localize()).collect::<Vec<_>>()
        );
        for diagnostic in got {
            assert_eq!(diagnostic.category(), category, "{name}: wrong category");
            assert!(
                diagnostic.file().is_some_and(|file| file.file_name() == "/tsconfig.json"),
                "{name}: missing local config location"
            );
            assert_eq!(underline(diagnostic), r#""floatingEfect""#, "{name}: wrong underline");
        }
    }
}

// go test: internal/effectconfigcheck/validation_test.go:105 TestDeprecatedRuleNames f04b431363bfda75
#[test]
fn test_deprecated_rule_names() {
    for (name, options, count, category) in [
        (
            "deprecated",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning"}"#,
            1,
            Category::Warning,
        ),
        (
            "deprecated disabled rule still reported",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"off"}"#,
            1,
            Category::Warning,
        ),
        (
            "replacements",
            r#""diagnosticSeverity":{"globalJson":"warning","globalJsonInEffect":"warning"}"#,
            0,
            Category::Warning,
        ),
        (
            "error",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning","deprecatedRuleName":"error"}"#,
            1,
            Category::Error,
        ),
        (
            "off",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning","deprecatedRuleName":"off"}"#,
            0,
            Category::Warning,
        ),
        (
            "unknownRuleName off",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning","unknownRuleName":"off"}"#,
            1,
            Category::Warning,
        ),
        (
            "disabled",
            r#""diagnostics":false,"diagnosticSeverity":{"preferSchemaOverJson":"warning"}"#,
            0,
            Category::Warning,
        ),
        (
            "override",
            r#""overrides":[{"options":{"diagnosticSeverity":{"preferSchemaOverJson":"warning"}}}]"#,
            1,
            Category::Warning,
        ),
        (
            "severity and override",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning"},"overrides":[{"options":{"diagnosticSeverity":{"preferSchemaOverJson":"off"}}}]"#,
            2,
            Category::Warning,
        ),
    ] {
        let host = new_host(&[("/tsconfig.json", &config(options, ""))]);
        let parsed = parse(host, "/tsconfig.json", None);
        let got = deprecated_diagnostics(&parsed);
        assert_eq!(
            got.len(),
            count,
            "{name}: want {count} warnings, got {}: {:?}",
            got.len(),
            parsed.errors.iter().map(|d| d.localize()).collect::<Vec<_>>()
        );
        assert!(
            unknown_diagnostics(&parsed).is_empty(),
            "{name}: deprecated rule reported as unknown: {:?}",
            parsed.errors.iter().map(|d| d.localize()).collect::<Vec<_>>()
        );
        for diagnostic in got {
            assert_eq!(diagnostic.category(), category, "{name}: wrong category");
            assert!(
                diagnostic.file().is_some_and(|file| file.file_name() == "/tsconfig.json"),
                "{name}: missing local config location"
            );
            assert_eq!(
                underline(diagnostic),
                r#""preferSchemaOverJson""#,
                "{name}: wrong underline"
            );
            let args = diagnostic.message_args();
            assert!(
                args.len() == 2
                    && args[0] == "preferSchemaOverJson"
                    && args[1] == "`globalJsonInEffect` or `globalJson`",
                "{name}: wrong message arguments: {args:?}"
            );
        }
    }
}

// go test: internal/effectconfigcheck/validation_test.go:151 TestDeprecatedRuleNamesExtends 9112fd26c73f659d
#[test]
fn test_deprecated_rule_names_extends() {
    for (name, base, child, count, category, local) in [
        (
            "inherited key",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning"}"#,
            r#""diagnosticSeverity":{}"#,
            1,
            Category::Warning,
            false,
        ),
        (
            "child silences base",
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning"}"#,
            r#""diagnosticSeverity":{"deprecatedRuleName":"off"}"#,
            0,
            Category::Warning,
            false,
        ),
        (
            "base raises child",
            r#""diagnosticSeverity":{"deprecatedRuleName":"error"}"#,
            r#""diagnosticSeverity":{"preferSchemaOverJson":"warning"}"#,
            1,
            Category::Error,
            true,
        ),
    ] {
        let host = new_host(&[
            ("/base.json", &config(base, "")),
            ("/tsconfig.json", &config(child, r#","extends":"./base.json""#)),
        ]);
        let cache = extended_config_cache();
        let parsed = parse(host, "/tsconfig.json", Some(&cache));
        let got = deprecated_diagnostics(&parsed);
        assert_eq!(
            got.len(),
            count,
            "{name}: want {count} warnings, got {}: {:?}",
            got.len(),
            parsed.errors.iter().map(|d| d.localize()).collect::<Vec<_>>()
        );
        for d in got {
            assert_eq!(d.category(), category, "{name}: wrong category");
            assert_eq!(d.file().is_some(), local, "{name}: wrong inherited/local location");
        }
    }
}

// go test: internal/effectconfigcheck/validation_test.go:186 TestUnknownRuleNamesExtends cb950712265e3b29
#[test]
fn test_unknown_rule_names_extends() {
    for (name, base, child, count, category, local) in [
        (
            "inherited key",
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            r#""diagnosticSeverity":{}"#,
            1,
            Category::Warning,
            false,
        ),
        (
            "child silences base",
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            r#""diagnosticSeverity":{"unknownRuleName":"off"}"#,
            0,
            Category::Warning,
            false,
        ),
        (
            "child disables diagnostics",
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            r#""diagnostics":false"#,
            0,
            Category::Warning,
            false,
        ),
        (
            "child raises base",
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            r#""diagnosticSeverity":{"unknownRuleName":"error"}"#,
            1,
            Category::Error,
            false,
        ),
        (
            "base silences child",
            r#""diagnosticSeverity":{"unknownRuleName":"off"}"#,
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            0,
            Category::Warning,
            true,
        ),
        (
            "base disables child",
            r#""diagnostics":false"#,
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            0,
            Category::Warning,
            true,
        ),
        (
            "base raises child",
            r#""diagnosticSeverity":{"unknownRuleName":"error"}"#,
            r#""diagnosticSeverity":{"floatingEfect":"error"}"#,
            1,
            Category::Error,
            true,
        ),
    ] {
        let host = new_host(&[
            ("/base.json", &config(base, "")),
            ("/middle.json", r#"{"extends":"./base.json"}"#),
            ("/tsconfig.json", &config(child, r#","extends":"./middle.json""#)),
        ]);
        let cache = extended_config_cache();
        let parsed = parse(host, "/tsconfig.json", Some(&cache));
        let got = unknown_diagnostics(&parsed);
        assert_eq!(got.len(), count, "{name}: want {count} warnings, got {}", got.len());
        for d in got {
            assert_eq!(d.category(), category, "{name}: wrong category");
            assert_eq!(d.file().is_some(), local, "{name}: wrong inherited/local location");
        }
    }
}

// go test: internal/effectconfigcheck/validation_test.go:226 TestEffectPluginInheritance 1a65f2b80fbdc197
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
        let cache = extended_config_cache();
        let parsed = parse(new_host(&files), "/tsconfig.json", Some(&cache));
        let effect = parsed.compiler_options().and_then(|options| options.effect.get());
        if active {
            let effect =
                effect.unwrap_or_else(|| panic!("{name}: expected Effect plugin to be active"));
            assert!(!effect.diagnostics, "{name}: expected inherited diagnostics=false");
            continue;
        }
        assert!(effect.is_none(), "{name}: expected Effect plugin to be inactive, got {effect:?}");
    }
}

// go test: internal/effectconfigcheck/validation_test.go:284 TestInheritedOverridesUseLocalSyntaxOnly 57ea5abcd4c79e92
#[test]
fn test_inherited_overrides_use_local_syntax_only() {
    let host = new_host(&[
        (
            "/base.json",
            &config(
                r#""overrides":[{"options":{"diagnosticSeverity":{"inheritedTypo":"warning"}}}]"#,
                "",
            ),
        ),
        (
            "/tsconfig.json",
            &config(
                r#""overrides":[false,{"options":{"diagnosticSeverity":{"localTypo":"warning"}}}]"#,
                r#","extends":"./base.json""#,
            ),
        ),
    ]);
    let got = unknown_diagnostics(&parse(host, "/tsconfig.json", None));
    assert_eq!(got.len(), 2, "want two warnings, got {}", got.len());
    assert!(
        got[0].file().is_none(),
        "inherited override must not be attributed to a local override"
    );
    assert!(got[1].file().is_some(), "local override must have a location");
    assert_eq!(underline(got[1]), r#""localTypo""#, "wrong local underline");
}

// go test: internal/effectconfigcheck/validation_test.go:305 TestUnknownRuleNamesJSONAPI 512ddd9baf6a637e
#[test]
fn test_unknown_rule_names_json_api() {
    // Use the compiler's JSON representation, which both providers accept.
    let (raw, errors) = ets_shim::tsoptions::parse_config_file_text_to_json(
        "/tsconfig.json",
        tspath::to_path("/tsconfig.json", "", true),
        &config(r#""diagnosticSeverity":{"floatingEfect":"error"}"#, ""),
    );
    assert!(errors.is_empty(), "invalid config fixture");
    let parsed = ets_shim::tsoptions::parse_json_config_file_content(
        raw,
        new_host(&[]),
        "/",
        None,
        "/tsconfig.json",
        &[],
        None,
    );
    let got = unknown_diagnostics(&parsed);
    assert!(
        got.len() == 1 && got[0].file().is_none(),
        "expected one locationless warning: {:?}",
        got.iter().map(|d| d.localize()).collect::<Vec<_>>()
    );
}

// go test: internal/effectconfigcheck/validation_test.go:319 TestProjectReferencesAndSharedExtends 44252f2a0210ea18
#[test]
fn test_project_references_and_shared_extends() {
    let host = new_host(&[
        ("/base.json", &config(r#""diagnosticSeverity":{"floatingEfect":"error"}"#, "")),
        (
            "/tsconfig.json",
            r#"{"files":[],"references":[{"path":"./a"},{"path":"./b"},{"path":"./c"}]}"#,
        ),
        (
            "/a/tsconfig.json",
            &config(
                r#""diagnosticSeverity":{"unknownRuleName":"off"}"#,
                r#","extends":"../base.json""#,
            ),
        ),
        (
            "/b/tsconfig.json",
            &config(
                r#""diagnosticSeverity":{"unknownRuleName":"error"}"#,
                r#","extends":"../base.json""#,
            ),
        ),
        ("/c/tsconfig.json", r#"{"extends":"../base.json"}"#),
    ]);
    let cache: Arc<ets_shim::execute::tsc::ExtendedConfigCache> = Arc::new(extended_config_cache());
    let root = parse(host, "/tsconfig.json", Some(&*cache));
    assert!(
        unknown_diagnostics(&root).is_empty(),
        "reference diagnostics leaked into solution config"
    );
    let mut program_options = ProgramOptions::new(
        P::new(root),
        ets_shim::compiler::new_compiler_host(
            "",
            Arc::clone(&host.fs),
            &bundled::lib_path(),
            Some(Arc::clone(&cache) as Arc<dyn ExtendedConfigCache + Send>),
            None,
            None,
        ),
    );
    program_options.single_threaded = Tristate::True;
    let program = ets_shim::compiler::new_program(program_options);
    let refs = program.get_resolved_project_references();
    assert_eq!(refs.len(), 3, "expected three resolved references, got {}", refs.len());
    for (i, ref_) in refs.iter().enumerate() {
        let Some(ref_) = ref_ else {
            panic!("reference {i} did not resolve");
        };
        let got = unknown_diagnostics(ref_);
        if i == 0 {
            assert!(got.is_empty(), "project a did not silence inherited typo");
            continue;
        }
        let category = if i == 1 { Category::Error } else { Category::Warning };
        assert!(
            got.len() == 1 && got[0].category() == category && got[0].file().is_none(),
            "project {i}: unexpected diagnostics {:?}",
            got.iter().map(|d| d.localize()).collect::<Vec<_>>()
        );
    }
    // Parsing the base through the same cache must not reuse a child's severity.
    let got = unknown_diagnostics(&parse(host, "/base.json", Some(&*cache)));
    assert!(got.len() == 1 && got[0].category() == Category::Warning, "cached severity leaked");
}

// go test: internal/effectconfigcheck/validation_test.go:368 TestMultipleExtendsAndExistingOptions 0e777470169add38
#[test]
fn test_multiple_extends_and_existing_options() {
    let host = new_host(&[
        ("/base.json", &config(r#""diagnosticSeverity":{"floatingEfect":"error"}"#, "")),
        ("/severity.json", &config(r#""diagnosticSeverity":{"unknownRuleName":"error"}"#, "")),
        ("/tsconfig.json", r#"{"extends":["./base.json","./severity.json"]}"#),
    ]);
    let got = unknown_diagnostics(&parse(host, "/tsconfig.json", None));
    assert!(got.len() == 1 && got[0].category() == Category::Error, "multiple extends failed");
    let text = host.fs.read_file("/tsconfig.json").unwrap();
    let source = new_tsconfig_source_file_from_file_path(
        "/tsconfig.json",
        tspath::to_path("/tsconfig.json", "", true),
        &text,
    );
    let existing = CompilerOptions {
        effect: Some(Arc::new(EffectPluginOptions {
            diagnostics: true,
            diagnostic_severity: Some(FxHashMap::from_iter([(
                "existingTypo".to_string(),
                Severity::Error,
            )])),
            ..Default::default()
        }))
        .into(),
        ..Default::default()
    };
    let parsed = parse_json_source_file_config_file_content(
        source,
        host,
        "/",
        Some(&existing),
        None,
        "/tsconfig.json",
        &[],
        None,
    );
    let got = unknown_diagnostics(&parsed);
    assert!(
        got.len() == 1 && got[0].message_args().join(" ").contains("existingTypo"),
        "existing options were not validated after merging"
    );
}

// go test: internal/effectconfigcheck/validation_test.go:389 TestOtherPluginsAndCompilerErrors 235be75931db41d9
#[test]
fn test_other_plugins_and_compiler_errors() {
    let host = new_host(&[(
        "/tsconfig.json",
        r#"{"files":["/main.ts"],"compilerOptions":{"strcit":true,"plugins":[{"name":"other-plugin","diagnosticSeverity":{"unrelatedRule":"error"}}]}}"#,
    )]);
    let parsed = parse(host, "/tsconfig.json", None);
    assert!(unknown_diagnostics(&parsed).is_empty(), "validated an unrelated plugin");
    assert_eq!(
        parsed.errors.len(),
        1,
        "expected the existing compiler-option diagnostic, got {:?}",
        parsed.errors.iter().map(|d| d.localize()).collect::<Vec<_>>()
    );
}

// go test: internal/effectconfigcheck/validation_test.go:403 TestUnknownRuleNamesDeterministicOrder 8d2d51201ffb4f0f
#[test]
fn test_unknown_rule_names_deterministic_order() {
    let host = new_host(&[(
        "/tsconfig.json",
        &config(r#""diagnosticSeverity":{"zTypo":"warning","aTypo":"error","mTypo":"off"}"#, ""),
    )]);
    let got = unknown_diagnostics(&parse(host, "/tsconfig.json", None));
    assert_eq!(got.len(), 3, "expected three warnings, got {}", got.len());
    for (i, name) in ["aTypo", "mTypo", "zTypo"].into_iter().enumerate() {
        let args = got[i].message_args();
        assert!(args.len() == 1 && args[0] == name, "wrong diagnostic order: {args:?}");
    }
}
