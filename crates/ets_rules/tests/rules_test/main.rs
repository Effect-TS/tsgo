//! Rust-only: the test binary of the ported `internal/rules` external Go tests (`package rules_test`), one module per
//! Go file; this file holds `rules_json_test.go`.
//!
//! Port of `internal/rules/rules_json_test.go`: the README diagnostics table and example
//! config, `_packages/tsgo/src/metadata.json` and the generated `docs/rules/*.md` pages.
//!
//! Rust: Go's `encoding/json` output is reproduced by a small Rust-only encoder (`Json`); Go's reflection over
//! `etscore.EffectPluginOptions` struct tags is an explicit table (`EFFECT_PLUGIN_OPTION_FIELDS`). Go's blank import
//! of `etscheckerhooks` is unneeded: `evaluatePreview` runs the rule directly.

mod effect_sleep_zero_to_yield_now_test;
mod fail_cause_to_constructor_test;
mod flat_map_identity_to_flatten_test;
mod flat_map_to_tap_test;
mod manual_metric_tap_to_track_test;
mod map_const_to_as_test;
mod map_map_error_to_map_both_test;
mod provide_merge_all_to_array_test;
mod type_union_to_schema_union_test;

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use ets_bundledeffect::EffectVersion;
use ets_etscore::Severity;
use ets_go::testing::T;
use ets_rule::{MetadataGroup, MetadataPreset, Rule};
use ets_shim::ast::SourceFileParseOptions;
use ets_shim::compiler::{ProgramOptions, ProgramRegions};
use ets_shim::core::arena::Region;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::{self, ComparePathsOptions};
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, NewLineKind, P, ScriptKind, ScriptTarget,
    Tristate,
};
use ets_shim::tsoptions::{ParseConfigHost, TsConfigSourceFile};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use regex::Regex;
use rustc_hash::FxHashMap;

// go test: internal/rules/rules_json_test.go:43 TestUpdateReadme 73e5c7da3d19267e
#[test]
fn test_update_readme() {
    ets_go::testing::run(|t| {
        t.parallel();
        if std::env::var("UPDATE_README").unwrap_or_default().is_empty() {
            t.skip("set UPDATE_README=1 to regenerate README.md");
        }
        let root = repo_root(t);
        let readme_path = root.join("README.md");
        let committed = match std::fs::read(&readme_path) {
            Ok(committed) => committed,
            Err(err) => t.fatal(format!("read README.md: {err}")),
        };
        let generated = match generate_readme(&committed) {
            Ok(generated) => generated,
            Err(err) => t.fatal(format!("generate README: {err}")),
        };
        if let Err(err) = std::fs::write(&readme_path, generated) {
            t.fatal(format!("write README.md: {err}"));
        }
        t.log("README.md updated");
    });
}

// go test: internal/rules/rules_json_test.go:64 TestReadmeTable afb08894ca6c1e9e
#[test]
fn test_readme_table() {
    ets_go::testing::run(|t| {
        t.parallel();
        let root = repo_root(t);
        let local_path = root.join("testdata").join("baselines").join("local").join("README.md");
        let reference_path = root.join("README.md");

        let committed = match std::fs::read(&reference_path) {
            Ok(committed) => committed,
            Err(err) => t.fatal(format!("read README.md: {err}")),
        };

        let got = match generate_readme(&committed) {
            Ok(got) => got,
            Err(err) => t.fatal(format!("generate README: {err}")),
        };

        if got != committed {
            if let Err(err) = std::fs::create_dir_all(local_path.parent().unwrap()) {
                t.fatal(format!("create local baseline dir: {err}"));
            }
            if let Err(err) = std::fs::write(&local_path, &got) {
                t.fatal(format!("write local baseline: {err}"));
            }
            t.fatal(format!(
                "README.md diagnostics table mismatch:\nlocal: {}\nreference: {}",
                local_path.display(),
                reference_path.display()
            ));
        }
    });
}

// go test: internal/rules/rules_json_test.go:91 TestReadmeLinksAreAbsolute 1596dbee3b4b09d7
#[test]
fn test_readme_links_are_absolute() {
    ets_go::testing::run(|t| {
        t.parallel();
        let readme_path = repo_root(t).join("README.md");
        let content = match std::fs::read_to_string(&readme_path) {
            Ok(content) => content,
            Err(err) => t.fatal(format!("read README.md: {err}")),
        };

        let patterns = [
            Regex::new(r#"href="([^"]+)""#).unwrap(),
            Regex::new(r"\[[^\]]+\]\(([^)]+)\)").unwrap(),
        ];
        for pattern in &patterns {
            for m in pattern.captures_iter(&content) {
                let link = &m[1];
                if !link.starts_with("https://") && !link.starts_with("http://") {
                    t.error(format!(
                        "README.md contains non-absolute link {}",
                        ets_go::strconv::quote(link)
                    ));
                }
            }
        }
    });
}

// go test: internal/rules/rules_json_test.go:112 TestMetadataJSON f7b3797549f4ecf9
#[test]
fn test_metadata_json() {
    run_on_checker_stack(|t| {
        t.parallel();
        let root = repo_root(t);
        let local_path =
            root.join("testdata").join("baselines").join("local").join("metadata.json");
        let reference_path = root.join("_packages").join("tsgo").join("src").join("metadata.json");

        let got = marshal_metadata_json(t);

        let want = match std::fs::read(&reference_path) {
            Ok(want) => want,
            Err(err) => t.fatal(format!(
                "read reference metadata.json at {}: {err}",
                reference_path.display()
            )),
        };
        if got != want {
            if let Err(err) = std::fs::create_dir_all(local_path.parent().unwrap()) {
                t.fatal(format!("create local baseline dir: {err}"));
            }
            if let Err(err) = std::fs::write(&local_path, &got) {
                t.fatal(format!("write local baseline: {err}"));
            }
            t.fatal(format!(
                "metadata.json mismatch:\nlocal: {}\nreference: {}",
                local_path.display(),
                reference_path.display()
            ));
        }
    });
}

// go test: internal/rules/rules_json_test.go:138 TestUpdateMetadataJSON 2518cdc436546bb3
#[test]
fn test_update_metadata_json() {
    run_on_checker_stack(|t| {
        t.parallel();
        if std::env::var("UPDATE_METADATA_JSON").unwrap_or_default().is_empty() {
            t.skip("set UPDATE_METADATA_JSON=1 to regenerate metadata.json");
        }
        let root = repo_root(t);
        let metadata_path = root.join("_packages").join("tsgo").join("src").join("metadata.json");
        let generated = marshal_metadata_json(t);
        if let Err(err) = std::fs::write(&metadata_path, generated) {
            t.fatal(format!("write metadata.json: {err}"));
        }
        t.log("metadata.json updated");
    });
}

// go test: internal/rules/rules_json_test.go:155 TestRuleDocs ce39198229a3d412
#[test]
fn test_rule_docs() {
    ets_go::testing::run(|t| {
        t.parallel();
        let root = repo_root(t);
        let docs_path = root.join("docs").join("rules");
        let metadata = read_metadata_document(t, &root);
        let generated = generate_rule_docs(&metadata);

        let entries = match read_dir_sorted(&docs_path) {
            Ok(entries) => entries,
            Err(err) => t.fatal(format!("read generated rule docs: {err}")),
        };
        for (name, want) in &generated {
            let got = match std::fs::read_to_string(docs_path.join(name)) {
                Ok(got) => got,
                Err(err) => t.fatal(format!("read generated rule doc {name}: {err}")),
            };
            if got != *want {
                t.fatal(format!(
                    "generated rule doc {name} is out of date; run UPDATE_RULE_DOCS=1 go test ./internal/rules -run TestUpdateRuleDocs"
                ));
            }
        }
        for (name, is_dir) in &entries {
            if *is_dir || Path::new(name).extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            if !generated.contains_key(name) {
                t.fatal(format!("stale generated rule doc {name}"));
            }
        }
        for (path, heading) in [
            (root.join("README.md"), "## Installation"),
            (root.join("docs").join("README.md"), "# Oxlint Setup"),
        ] {
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(err) => t.fatal(format!("read rule docs guide {}: {err}", path.display())),
            };
            if !content.contains(heading) {
                t.fatal(format!(
                    "rule docs guide {} is missing heading {}",
                    path.display(),
                    ets_go::strconv::quote(heading)
                ));
            }
        }
    });
}

// go test: internal/rules/rules_json_test.go:200 TestAnnotatePreview e6f2e188896cfa35
#[test]
fn test_annotate_preview() {
    ets_go::testing::run(|t| {
        t.parallel();
        let got = annotate_preview(
            "const value = bad\n",
            &[PreviewDiagnostic {
                start: 14,
                end: 17,
                text: "This value is not allowed. effect(exampleRule)".to_string(),
            }],
            "exampleRule",
        );
        let want = "const value = bad\n/**\n              ^^^ effecttsgo(example-rule): This value is not allowed.\n*/";
        if got != want {
            t.fatal(format!("annotated preview mismatch\ngot:\n{got}\nwant:\n{want}"));
        }

        let got = annotate_preview(
            "é bad(\n  value\n)",
            &[PreviewDiagnostic {
                start: 2,
                end: 14,
                text: "This spans lines. effect(exampleRule)".to_string(),
            }],
            "exampleRule",
        );
        let want = "é bad(\n/**\n  ^^^^ effecttsgo(example-rule): This spans lines.\n*/\n  value\n/**\n^^^^^^^\n*/\n)";
        if got != want {
            t.fatal(format!("multiline annotated preview mismatch\ngot:\n{got}\nwant:\n{want}"));
        }
    });
}

// go test: internal/rules/rules_json_test.go:223 TestMarkdownCodeFence 11529b39cea59da9
#[test]
fn test_markdown_code_fence() {
    ets_go::testing::run(|t| {
        t.parallel();
        let (got, want) = (markdown_code_fence("const value = \"```\""), "````");
        if got != want {
            t.fatal(format!(
                "markdown fence mismatch: got {}, want {}",
                ets_go::strconv::quote(&got),
                ets_go::strconv::quote(want)
            ));
        }
    });
}

// go test: internal/rules/rules_json_test.go:230 TestUpdateRuleDocs 6f6742ce78dfff78
#[test]
fn test_update_rule_docs() {
    run_on_checker_stack(|t| {
        if std::env::var("UPDATE_RULE_DOCS").unwrap_or_default().is_empty() {
            t.skip("set UPDATE_RULE_DOCS=1 to regenerate docs/rules");
        }
        let root = repo_root(t);
        let docs_path = root.join("docs").join("rules");
        if let Err(err) = std::fs::create_dir_all(&docs_path) {
            t.fatal(format!("create rule docs directory: {err}"));
        }
        let generated = generate_rule_docs(&generate_metadata_document(t));
        let entries = match read_dir_sorted(&docs_path) {
            Ok(entries) => entries,
            Err(err) => t.fatal(format!("read rule docs directory: {err}")),
        };
        for (name, is_dir) in &entries {
            if !*is_dir && Path::new(name).extension().is_some_and(|ext| ext == "md") {
                if let Err(err) = std::fs::remove_file(docs_path.join(name)) {
                    t.fatal(format!("remove stale rule doc {name}: {err}"));
                }
            }
        }
        let mut names: Vec<&String> = generated.keys().collect();
        names.sort();
        for name in names {
            if let Err(err) = std::fs::write(docs_path.join(name), &generated[name]) {
                t.fatal(format!("write generated rule doc {name}: {err}"));
            }
        }
        t.log(format!("updated {} rule docs", generated.len()));
    });
}

/// Rust-only: runs a test body that type checks on a thread with the checker's stack
/// (`ets_go::testing::on_checker_stack`).
fn run_on_checker_stack(f: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || ets_go::testing::run(f));
}

/// Rust-only: Go's `os.ReadDir` (entries sorted by name): `(name, is directory)`.
fn read_dir_sorted(dir: &Path) -> std::io::Result<Vec<(String, bool)>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        entries
            .push((entry.file_name().to_string_lossy().into_owned(), entry.file_type()?.is_dir()));
    }
    entries.sort();
    Ok(entries)
}

// go test: internal/rules/rules_json_test.go:260 repoRoot 2c97efbb4d978e25
// Rust: Go resolves the repository root from this file's path (`runtime.Caller`); cargo gives the crate directory.
fn repo_root(t: &T) -> PathBuf {
    t.helper();
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    match crate_dir.join("..").join("..").canonicalize() {
        Ok(root) => root,
        Err(_) => t.fatal("resolve caller path"),
    }
}

// go test: internal/rules/rules_json_test.go:269 previewDiagnostic c6d7a047d83601c5
#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviewDiagnostic {
    start: i64,   // json:"start"
    end: i64,     // json:"end"
    text: String, // json:"text"
}

// go test: internal/rules/rules_json_test.go:275 previewPayload 139d50d898f1cd05
#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviewPayload {
    source_text: String,                 // json:"sourceText"
    diagnostics: Vec<PreviewDiagnostic>, // json:"diagnostics"
}

// go test: internal/rules/rules_json_test.go:280 exportedRule 51fd03f5b14d1c29
#[derive(Clone, Debug, PartialEq, Eq)]
struct ExportedRule {
    name: String,                    // json:"name"
    group: String,                   // json:"group"
    description: String,             // json:"description"
    default_severity: Severity,      // json:"defaultSeverity"
    fixable: bool,                   // json:"fixable"
    supported_effect: Vec<String>,   // json:"supportedEffect"
    codes: Vec<i32>,                 // json:"codes"
    replaced_by: Vec<String>,        // json:"replacedBy,omitempty"
    preview: Option<PreviewPayload>, // json:"preview,omitempty"
}

// go test: internal/rules/rules_json_test.go:292 metadataDocument 9b843e81a3ff4565
//
// Rust: `rule.MetadataGroup` / `rule.MetadataPreset` hold `&'static str`; a decoded document leaks its strings
// (tests only).
#[derive(Clone, Debug)]
struct MetadataDocument {
    groups: Vec<MetadataGroup>,   // json:"groups"
    presets: Vec<MetadataPreset>, // json:"presets"
    rules: Vec<ExportedRule>,     // json:"rules"
}

// go test: internal/rules/rules_json_test.go:299 buildFixableCodes 10c107a8a05019ff
// buildFixableCodes returns a set of diagnostic codes that have non-disable fixables.
fn build_fixable_codes() -> FxHashMap<i32, bool> {
    let mut result = FxHashMap::default();
    for f in ets_fixables::ALL.iter() {
        if f.name == "effectDisable" {
            continue;
        }
        for code in &f.error_codes {
            result.insert(*code, true);
        }
    }
    result
}

// go test: internal/rules/rules_json_test.go:313 isRuleFixable 9177b661fca7a70c
// isRuleFixable returns true if any of the rule's codes has a non-disable fixable.
fn is_rule_fixable(codes: &[i32], fixable_codes: &FxHashMap<i32, bool>) -> bool {
    for code in codes {
        if fixable_codes.get(code).copied().unwrap_or_default() {
            return true;
        }
    }
    false
}

// go test: internal/rules/rules_json_test.go:325 trimLeadingDirectives 918eeef72c90c629
// trimLeadingDirectives strips leading lines starting with "// @" from sourceText
// and returns the trimmed text along with the number of characters removed (including newlines).
// This matches the upstream trimLeadingDirectives logic used for preview generation.
fn trim_leading_directives(source_text: &str) -> (String, usize) {
    let lines: Vec<&str> = source_text.split('\n').collect();
    let mut index = 0;
    let mut removed_chars = 0;

    while index < lines.len() {
        if !lines[index].starts_with("// @") {
            break;
        }
        removed_chars += lines[index].len();
        if index < lines.len() - 1 {
            removed_chars += 1; // newline character
        }
        index += 1;
    }

    (lines[index..].join("\n"), removed_chars)
}

// go test: internal/rules/rules_json_test.go:345 findPreviewFile 0cc3571263fcc58c
// findPreviewFile locates the preview fixture for a rule, checking v4 first then v3.
// Returns the version, file path, and source text.
fn find_preview_file(
    root: &Path,
    rule_name: &str,
) -> Result<(EffectVersion, PathBuf, String), String> {
    for version in [EffectVersion::V4, EffectVersion::V3] {
        let file_path = root
            .join("testdata")
            .join("tests")
            .join(version.as_str())
            .join(format!("{rule_name}_preview.ts"));
        if let Ok(data) = std::fs::read_to_string(&file_path) {
            return Ok((version, file_path, data));
        }
    }
    Err(format!("no preview file found for rule {rule_name}"))
}

// go test: internal/rules/rules_json_test.go:358 parseTestConfig 2ab75116baa2a78c
// parseTestConfig extracts a @test-config JSON object from source comments.
// Returns a map of extra config to merge into the Effect plugin options.
//
// Rust: RE2's ASCII `\s` is spelled out; the decoded object keeps Go's `map[string]any` semantics (sorted keys on
// encoding, `Json::Object` sorted here).
fn parse_test_config(source_text: &str) -> Option<Vec<(String, Json)>> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//[\t\n\f\r ]*@test-config[\t\n\f\r ]+(.+)").unwrap());
    let m = RE.captures(source_text)?;
    let value: serde_json::Value = serde_json::from_str(&m[1]).ok()?;
    match Json::from_decoded(&value) {
        Json::Object(fields) => Some(fields),
        _ => None,
    }
}

// go test: internal/rules/rules_json_test.go:373 buildTsConfigWithTestConfig 63fbaf283175003b
// buildTsConfigWithTestConfig creates a tsconfig JSON string that merges
// the default Effect plugin config with any @test-config overrides.
fn build_ts_config_with_test_config(test_config: Option<&[(String, Json)]>) -> String {
    let mut plugin: FxHashMap<String, Json> = FxHashMap::default();
    plugin.insert("name".to_string(), Json::String("@effect/language-service".to_string()));
    plugin.insert("ignoreEffectErrorsInTscExitCode".to_string(), Json::Bool(true));
    plugin.insert("skipDisabledOptimization".to_string(), Json::Bool(true));
    if let Some(test_config) = test_config {
        for (key, value) in test_config {
            plugin.insert(key.clone(), value.clone());
        }
    }
    let ts_config = Json::object_from_map(vec![(
        "compilerOptions".to_string(),
        Json::object_from_map(vec![(
            "plugins".to_string(),
            Json::Array(vec![Json::object_from_map(plugin)]),
        )]),
    )]);
    ts_config.marshal()
}

/// Rust-only: Go's `tsoptions.NewDefaultParseConfigHost(fs)`.
struct DefaultParseConfigHost {
    fs: Arc<dyn FS>,
    current_directory: String,
}

impl ParseConfigHost for DefaultParseConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }

    fn get_current_directory(&self) -> &str {
        &self.current_directory
    }
}

// go test: internal/rules/rules_json_test.go:398 evaluatePreview 70bd9e615b5893e2
// evaluatePreview creates an in-memory program for a preview fixture and runs
// the specified rule directly to collect diagnostics. We bypass the checker hooks
// because preview files use "// @effect-diagnostics *:off" which causes the
// hook to early-return. Instead, we run the rule directly via rule.Run().
//
// Rust: as `ets_effecttest::run_effect_test`, the program is built in regions and freed at the end (Go: the GC),
// and the parse-config host is leaked for the `&'static` tsoptions wants (tests only).
fn evaluate_preview(t: &T, version: EffectVersion, source_text: &str, r: &Rule) -> PreviewPayload {
    t.helper();

    ets_effecttest::acquire_program();
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            ets_effecttest::release_program();
        }
    }
    let _release = Release;

    // Parse test units (handles @filename directives for multi-file tests)
    let default_file_name = "preview.ts";
    let units = parse_preview_units(source_text, default_file_name);

    // Create VFS
    let mut testfs = ets_bundledeffect::TestFS::default();
    if let Err(err) = ets_bundledeffect::mount_effect(version, &mut testfs) {
        t.fatal(format!("mount effect for preview: {err}"));
    }
    if source_text.contains("vitest") {
        if let Err(err) = ets_bundledeffect::mount_vitest(version, &mut testfs) {
            t.fatal(format!("mount vitest for preview: {err}"));
        }
    }

    let current_directory = "/.src";

    let region = Region::new_scratch(1 << 20);
    let region_scope = region.enter();

    // Add test files to VFS
    let mut program_file_names: Vec<String> = Vec::new();

    for unit in &units {
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        testfs.insert(unit_name.clone(), MapFile::from(unit.content.as_str()));
        if unit_name.starts_with("/node_modules/") {
            continue;
        }
        program_file_names.push(unit_name);
    }

    // Inject tsconfig with optional @test-config overrides
    let test_config = parse_test_config(source_text);
    let ts_config_content = match &test_config {
        Some(test_config) => build_ts_config_with_test_config(Some(test_config)),
        None => ets_effecttest::DEFAULT_TS_CONFIG.to_string(),
    };
    let ts_config_name = tspath::get_normalized_absolute_path("tsconfig.json", current_directory);
    testfs.insert(ts_config_name.clone(), MapFile::from(ts_config_content.as_str()));
    let ts_config_path = tspath::to_path(&ts_config_name, current_directory, true);
    let config_json = ets_shim::parser::parse_source_file(
        SourceFileParseOptions {
            file_name: ts_config_name.clone(),
            path: ts_config_path,
            ..Default::default()
        },
        &ts_config_content,
        ScriptKind::JSON,
    );
    let ts_config_file = TsConfigSourceFile::new(config_json);

    // Create VFS
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(testfs, true)));

    // Setup compiler options
    let mut compiler_options = P::new(CompilerOptions {
        new_line: NewLineKind::LF,
        skip_default_lib_check: Tristate::True,
        no_error_truncation: Tristate::True,
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        es_module_interop: Tristate::True,
        allow_synthetic_default_imports: Tristate::True,
        ..CompilerOptions::default()
    });

    // Parse tsconfig
    let config_dir = tspath::get_directory_path("tsconfig.json");
    let config_dir = tspath::get_normalized_absolute_path(&config_dir, current_directory);
    let host = DefaultParseConfigHost {
        fs: Arc::clone(&fs),
        current_directory: current_directory.to_string(),
    };
    // Rust: Go's GC frees the host; a leaked host would keep the preview's file system (the mounted Effect package)
    // alive. SAFETY (lifetime only): tsoptions reads the host during the call and keeps nothing of it (TSRS_API.md,
    // "Gaps": `&'static dyn ParseConfigHost` for a non-static host).
    let host: &'static DefaultParseConfigHost = unsafe { &*std::ptr::from_ref(&host) };
    let parsed_config = ets_shim::tsoptions::parse_json_source_file_config_file_content(
        ts_config_file,
        host,
        &config_dir,
        None,
        None,
        &ts_config_name,
        &[],
        None,
    );
    if let Some(options) = parsed_config.compiler_options() {
        let mut options = (*options).clone();
        options.new_line = NewLineKind::LF;
        options.skip_default_lib_check = Tristate::True;
        options.no_error_truncation = Tristate::True;
        if options.target == ScriptTarget::NONE {
            options.target = ScriptTarget::ES_NEXT;
        }
        if options.module == ModuleKind::NONE {
            options.module = ModuleKind::NODE_NEXT;
        }
        if options.module_resolution == ModuleResolutionKind::UNKNOWN {
            options.module_resolution = ModuleResolutionKind::NODE_NEXT;
        }
        compiler_options = P::new(options);
    }

    // Create compiler host
    let compiler_host = ets_shim::compiler::new_compiler_host(
        current_directory,
        fs,
        &bundled::lib_path(),
        None,
        None,
        None,
    );

    // Create program
    let mut program_config = ets_shim::tsoptions::new_parsed_command_line(
        compiler_options,
        program_file_names.clone(),
        Vec::new(),
        ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: "/".to_string(),
        },
    );
    program_config.config_file = parsed_config.config_file;
    let mut program_options = ProgramOptions::new(P::new(program_config), compiler_host);
    program_options.single_threaded = Tristate::True;
    program_options.checker_regions = true;
    program_options.checker_diagnostic_regions = true;
    let loader_regions = Arc::new(ProgramRegions::default());
    program_options.regions = Some(Arc::clone(&loader_regions));
    let program = ets_shim::compiler::new_program(program_options);

    // Force full type-checking by calling GetSemanticDiagnostics first.
    // This ensures the checker processes all files, populating relation errors
    // and other type data needed by rules. The checker hooks won't emit Effect
    // diagnostics due to the *:off directive, but the type info will be available.
    let ctx = Context::background();
    drop(program.get_semantic_diagnostics(&ctx, None));

    // Now get the type checker and run the rule directly against each source file
    let mut rule_diags: Vec<PreviewDiagnostic> = Vec::new();
    {
        let mut c = program.get_type_checker(&ctx);

        for file_name in &program_file_names {
            let Some(sf) = program.get_source_file(file_name) else {
                continue;
            };
            if sf.is_declaration_file() {
                continue;
            }
            let program_options = program.options();
            let parsed_effect_config = program_options.effect.get();
            let options = parsed_effect_config.as_deref().and_then(|parsed_effect_config| {
                ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
                    Some(parsed_effect_config),
                    sf.file_name(),
                    &program_options.config_file_path,
                    program.use_case_sensitive_file_names(),
                )
            });
            let mut tp = ets_typeparser::new_type_parser(program, &mut c);
            let checker_program: &'static dyn ets_shim::checker::Program = program;
            let rule_context = Context::background();
            let mut rule_ctx = ets_rule::new_context(
                &rule_context,
                checker_program,
                &mut tp,
                sf,
                options.as_ref(),
                r.default_severity,
            );
            let diags = (r.run)(&mut rule_ctx);
            let position_map = sf.get_position_map();
            for diagnostic in diags {
                rule_diags.push(PreviewDiagnostic {
                    start: i64::from(
                        position_map.utf8_to_utf16(diagnostic.pos().unwrap_or_default()),
                    ),
                    end: i64::from(
                        position_map.utf8_to_utf16(diagnostic.end().unwrap_or_default()),
                    ),
                    text: diagnostic.localize(),
                });
            }
        }
    }

    // Sort by start position
    // Rust: Go's `sort.Slice` is unstable; ties keep the rule's emission order here (sort_by is stable).
    rule_diags.sort_by_key(|d| d.start);

    // Trim leading directives from source text
    let (trimmed_source, removed_chars) = trim_leading_directives(source_text);
    let removed_units = utf16_length(&source_text[..removed_chars]);

    // Build preview diagnostics with adjusted offsets
    let mut prev_diags = Vec::with_capacity(rule_diags.len());
    for d in &rule_diags {
        let mut start = d.start - removed_units;
        let mut end = d.end - removed_units;
        if start < 0 {
            start = 0;
        }
        if end < 0 {
            end = 0;
        }
        prev_diags.push(PreviewDiagnostic { start, end, text: d.text.clone() });
    }

    drop(region_scope);
    // SAFETY: a full build from `new_program`; every checker handle and everything read from it is gone (the
    // preview holds strings).
    unsafe { ets_shim::compiler::free_unshared_program(program) };
    drop(loader_regions);
    drop(region);

    PreviewPayload { source_text: trimmed_source, diagnostics: prev_diags }
}

// go test: internal/rules/rules_json_test.go:574 parsePreviewUnits eb3117cf2008aff1
// parsePreviewUnits parses a preview file into test units.
// Reuses the same logic as effecttest.parseTestUnits.
//
// Rust: RE2's ASCII `\s` and `\w` are spelled out.
fn parse_preview_units(content: &str, default_file_name: &str) -> Vec<PreviewUnit> {
    static OPTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^/{2}[\t\n\f\r ]*@([0-9A-Za-z_]+)[\t\n\f\r ]*:[\t\n\f\r ]*([^\r\n]*)")
            .unwrap()
    });
    let lines = content.split('\n');

    let mut units = Vec::new();
    let mut current_content = String::new();
    let mut current_file_name = String::new();

    for line in lines {
        if let Some(test_meta_data) = OPTION_REGEX.captures(line) {
            let meta_data_name = test_meta_data[1].to_ascii_lowercase();
            if meta_data_name == "filename" {
                if !current_file_name.is_empty() && !current_content.is_empty() {
                    units.push(PreviewUnit {
                        name: current_file_name.clone(),
                        content: current_content.clone(),
                    });
                }
                current_file_name = ets_go::strings::trim_space(&test_meta_data[2]).to_string();
                current_content.clear();
                continue;
            }
        }
        if !current_content.is_empty() {
            current_content.push('\n');
        }
        current_content.push_str(line);
    }

    if !current_file_name.is_empty() {
        units.push(PreviewUnit { name: current_file_name, content: current_content });
    } else if !current_content.is_empty() {
        units.push(PreviewUnit { name: default_file_name.to_string(), content: current_content });
    }

    units
}

// go test: internal/rules/rules_json_test.go:619 previewUnit 37144eea3489adbe
struct PreviewUnit {
    name: String,
    content: String,
}

// go test: internal/rules/rules_json_test.go:624 marshalMetadataJSON 3a5e211b09645093
//
// Rust: Go's `([]byte, error)`: the Rust encoder cannot fail, so the bytes are returned directly.
fn marshal_metadata_json(t: &T) -> Vec<u8> {
    let root = repo_root(t);

    let groups = ets_rules::metadata_groups();
    let presets = ets_rules::metadata_presets();

    let fixable_codes = build_fixable_codes();

    let mut exported = Vec::with_capacity(ets_rules::ALL.len());
    for current in ets_rules::ALL.iter() {
        let mut codes = current.codes.clone();
        codes.sort_unstable();
        let fixable = is_rule_fixable(&codes, &fixable_codes);

        // Find and evaluate preview file
        let (version, _, source_text) = match find_preview_file(&root, current.name) {
            Ok(found) => found,
            Err(err) => t.fatal(err),
        };

        let preview = evaluate_preview(t, version, &source_text, current);

        exported.push(ExportedRule {
            name: current.name.to_string(),
            group: current.group.to_string(),
            description: current.description.to_string(),
            default_severity: current.default_severity,
            fixable,
            supported_effect: current.supported_effect.iter().map(|s| (*s).to_string()).collect(),
            codes,
            replaced_by: current.replaced_by.iter().map(|s| (*s).to_string()).collect(),
            preview: Some(preview),
        });
    }
    let mut group_order: FxHashMap<&str, usize> = FxHashMap::default();
    for (i, g) in groups.iter().enumerate() {
        group_order.insert(g.id, i);
    }
    // Rust: names are unique, so the unstable Go sort has no ties.
    exported.sort_by(|a, b| {
        let ga = group_order.get(a.group.as_str()).copied().unwrap_or_default();
        let gb = group_order.get(b.group.as_str()).copied().unwrap_or_default();
        ga.cmp(&gb).then_with(|| a.name.cmp(&b.name))
    });

    let doc = MetadataDocument { groups, presets, rules: exported };

    let mut data = doc.to_json().marshal_indent("", "  ").into_bytes();
    data.push(b'\n');
    data
}

// go test: internal/rules/rules_json_test.go:683 generateReadmeTable f96b3b7aca5cee3f
fn generate_readme_table() -> String {
    let groups = ets_rules::metadata_groups();

    struct RuleEntry {
        name: &'static str,
        group: &'static str,
        description: &'static str,
    }

    let mut all_rules = Vec::with_capacity(ets_rules::ALL.len());
    for r in ets_rules::ALL.iter() {
        all_rules.push(RuleEntry { name: r.name, group: r.group, description: r.description });
    }
    let mut group_order: FxHashMap<&str, usize> = FxHashMap::default();
    for (i, g) in groups.iter().enumerate() {
        group_order.insert(g.id, i);
    }
    // Rust: names are unique, so the unstable Go sort has no ties.
    all_rules.sort_by(|a, b| {
        let ga = group_order.get(a.group).copied().unwrap_or_default();
        let gb = group_order.get(b.group).copied().unwrap_or_default();
        ga.cmp(&gb).then_with(|| a.name.cmp(b.name))
    });

    let mut lines: Vec<String> = vec![
        "<table>".to_string(),
        "  <thead>".to_string(),
        "    <tr><th>Rule</th><th>Description</th></tr>".to_string(),
        "  </thead>".to_string(),
        "  <tbody>".to_string(),
    ];
    for group in &groups {
        let group_rules: Vec<&RuleEntry> =
            all_rules.iter().filter(|r| r.group == group.id).collect();
        if group_rules.is_empty() {
            continue;
        }
        lines.push(format!(
            "    <tr><td colspan=\"2\"><strong>{}</strong> <em>{}</em></td></tr>",
            html_escape_string(group.name),
            html_escape_string(group.description)
        ));
        for r in group_rules {
            lines.push(format!(
                "    <tr><td><a href=\"https://github.com/Effect-TS/tsgo/blob/main/docs/rules/{}.md\"><code>{}</code></a></td><td>{}</td></tr>",
                kebab_case(r.name),
                html_escape_string(r.name),
                html_escape_string(r.description)
            ));
        }
    }
    lines.push("  </tbody>".to_string());
    lines.push("</table>".to_string());
    lines.join("\n")
}

// go test: internal/rules/rules_json_test.go:739 readMetadataDocument e808a8ab54a424c5
fn read_metadata_document(t: &T, root: &Path) -> MetadataDocument {
    t.helper();
    let path = root.join("_packages").join("tsgo").join("src").join("metadata.json");
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(err) => t.fatal(format!("read metadata.json: {err}")),
    };
    match MetadataDocument::unmarshal(&data) {
        Ok(metadata) => metadata,
        Err(err) => t.fatal(format!("decode metadata.json: {err}")),
    }
}

// go test: internal/rules/rules_json_test.go:753 generateMetadataDocument fa25f0b4c3508e84
fn generate_metadata_document(t: &T) -> MetadataDocument {
    t.helper();
    let data = marshal_metadata_json(t);
    match MetadataDocument::unmarshal(&String::from_utf8_lossy(&data)) {
        Ok(metadata) => metadata,
        Err(err) => t.fatal(format!("decode generated metadata.json: {err}")),
    }
}

// go test: internal/rules/rules_json_test.go:766 generateRuleDocs ab7a34bf797343ef
fn generate_rule_docs(metadata: &MetadataDocument) -> FxHashMap<String, String> {
    let mut group_names: FxHashMap<&str, &str> = FxHashMap::default();
    for group in &metadata.groups {
        group_names.insert(group.id, group.name);
    }
    let mut docs = FxHashMap::default();
    for current in &metadata.rules {
        let slug = kebab_case(&current.name);
        let codes: Vec<String> = current.codes.iter().map(|code| format!("`TS{code}`")).collect();
        let fixable = if current.fixable { "Yes" } else { "No" };
        let mut page = String::new();
        page.push_str("<!-- This file is generated by internal/rules/rules_json_test.go. Do not edit it manually. -->\n\n");
        push_fmt(&mut page, format_args!("# `{}`\n\n{}\n\n", current.name, current.description));
        if !current.replaced_by.is_empty() {
            let replacements: Vec<String> = current
                .replaced_by
                .iter()
                .map(|name| format!("[`{name}`]({}.md)", kebab_case(name)))
                .collect();
            push_fmt(
                &mut page,
                format_args!(
                    "> [!WARNING]\n> This rule is deprecated. Use {} instead.\n\n",
                    replacements.join(" and ")
                ),
            );
        }
        page.push_str("| Property | Value |\n| --- | --- |\n");
        push_fmt(
            &mut page,
            format_args!(
                "| Category | {} |\n",
                markdown_table_cell(
                    group_names.get(current.group.as_str()).copied().unwrap_or_default()
                )
            ),
        );
        push_fmt(
            &mut page,
            format_args!("| Default severity | `{}` |\n", current.default_severity.string()),
        );
        push_fmt(&mut page, format_args!("| Fixable | {fixable} |\n"));
        push_fmt(
            &mut page,
            format_args!("| Effect versions | {} |\n", current.supported_effect.join(", ")),
        );
        push_fmt(&mut page, format_args!("| Diagnostic codes | {} |\n", codes.join(", ")));
        push_fmt(&mut page, format_args!("| Language Service name | `{}` |\n", current.name));
        push_fmt(&mut page, format_args!("| Oxlint name | `effecttsgo/{slug}` |\n"));

        page.push_str("\n## Preview\n\n");
        match &current.preview {
            None => page.push_str("No preview is available.\n"),
            Some(preview) => {
                let annotated =
                    annotate_preview(&preview.source_text, &preview.diagnostics, &current.name);
                let fence = markdown_code_fence(&annotated);
                push_fmt(&mut page, format_args!("{fence}ts\n{annotated}\n{fence}\n"));
                if preview.diagnostics.is_empty() {
                    page.push('\n');
                    page.push_str(
                        "This rule depends on project context and does not emit a diagnostic in the standalone preview.\n",
                    );
                }
            }
        }

        push_fmt(
            &mut page,
            format_args!(
                "\n## Language Service Configuration\n\nSee the [Language Service setup guide](../../README.md#installation) for installation instructions.\n\n```jsonc\n{{\n  \"$schema\": \"./node_modules/@effect/tsgo/schema.json\",\n  \"compilerOptions\": {{\n    \"plugins\": [\n      {{\n        \"name\": \"@effect/language-service\",\n        \"diagnosticSeverity\": {{\n          \"{}\": \"warning\"\n        }}\n      }}\n    ]\n  }}\n}}\n```\n",
                current.name
            ),
        );
        push_fmt(
            &mut page,
            format_args!(
                "\n## Oxlint Configuration\n\nSee the [Oxlint setup guide](../README.md#oxlint-setup) for installation and patching instructions.\n\n```json\n{{\n  \"$schema\": \"./node_modules/@effect/tsgo/oxlint-schema.json\",\n  \"options\": {{\n    \"typeAware\": true\n  }},\n  \"plugins\": [\"effecttsgo\"],\n  \"rules\": {{\n    \"effecttsgo/{slug}\": \"warn\"\n  }}\n}}\n```\n"
            ),
        );
        docs.insert(format!("{slug}.md"), page);
    }
    docs
}

// go test: internal/rules/rules_json_test.go:825 kebabCase f938b0bf8eff8561
fn kebab_case(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut result: Vec<u8> = Vec::with_capacity(bytes.len());
    for index in 0..bytes.len() {
        let current = bytes[index];
        if current.is_ascii_uppercase() {
            let previous_is_lower = index > 0 && bytes[index - 1].is_ascii_lowercase();
            let next_is_lower = index + 1 < bytes.len() && bytes[index + 1].is_ascii_lowercase();
            if index > 0 && (previous_is_lower || next_is_lower) {
                result.push(b'-');
            }
            result.push(current + (b'a' - b'A'));
        } else {
            result.push(current);
        }
    }
    String::from_utf8(result).expect("ASCII case mapping keeps UTF-8 valid")
}

// go test: internal/rules/rules_json_test.go:843 markdownTableCell b1fb2757ff2b7b0d
fn markdown_table_cell(value: &str) -> String {
    value.replace('\n', " ").replace('|', "\\|")
}

// go test: internal/rules/rules_json_test.go:847 annotatePreview 6c4f4b4d49898e58
fn annotate_preview(
    source_text: &str,
    diagnostics: &[PreviewDiagnostic],
    rule_name: &str,
) -> String {
    struct Annotation {
        padding: String,
        width: usize,
        text: String,
    }
    let lines: Vec<&str> = source_text.split('\n').collect();
    let mut line_starts = vec![0usize; lines.len()];
    let mut offset = 0;
    for (index, line) in lines.iter().enumerate() {
        line_starts[index] = offset;
        offset += line.len() + 1;
    }
    let mut annotations: FxHashMap<usize, Vec<Annotation>> = FxHashMap::default();
    let mut seen: FxHashMap<String, bool> = FxHashMap::default();
    let mut seen_messages: FxHashMap<String, bool> = FxHashMap::default();
    let mut sorted_diagnostics = diagnostics.to_vec();
    // Rust: Go's `slices.SortFunc` is unstable; ties (same start and end) differ only in text, and every message is
    // annotated in this order: stable sort, as Go's pdqsort keeps short equal runs in place.
    sorted_diagnostics
        .sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
    for diagnostic in &sorted_diagnostics {
        let start = utf16_offset_to_byte_index(source_text, diagnostic.start.max(0));
        let end =
            utf16_offset_to_byte_index(source_text, (diagnostic.start + 1).max(diagnostic.end));
        let text = diagnostic
            .text
            .strip_suffix(&format!(" effect({rule_name})"))
            .unwrap_or(&diagnostic.text);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut message_pending = !seen_messages.get(&text).copied().unwrap_or_default();
        seen_messages.insert(text.clone(), true);
        for (line_index, line) in lines.iter().enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let line_start = line_starts[line_index];
            let line_end = line_start + line.len();
            let segment_start = start.max(line_start);
            let segment_end = end.min(line_end);
            if segment_end <= segment_start {
                continue;
            }
            let byte_column = segment_start - line_start;
            let padding = annotation_padding(&line[..byte_column]);
            let width = line[byte_column..segment_end - line_start].chars().count().max(1);
            let mut annotation_text = String::new();
            if message_pending {
                annotation_text.clone_from(&text);
                message_pending = false;
            }
            let key = format!("{line_index}:{padding}:{width}:{annotation_text}");
            if seen.get(&key).copied().unwrap_or_default() {
                continue;
            }
            seen.insert(key, true);
            annotations.entry(line_index).or_default().push(Annotation {
                padding,
                width,
                text: annotation_text,
            });
        }
    }
    let mut result = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            result.push('\n');
        }
        result.push_str(line);
        for current in annotations.get(&index).map(Vec::as_slice).unwrap_or_default() {
            result.push_str("\n/**\n");
            result.push_str(&current.padding);
            result.push_str(&"^".repeat(current.width));
            if !current.text.is_empty() {
                push_fmt(
                    &mut result,
                    format_args!(" effecttsgo({}): {}", kebab_case(rule_name), current.text),
                );
            }
            result.push_str("\n*/");
        }
    }
    result.trim_matches(['\r', '\n']).to_string()
}

// go test: internal/rules/rules_json_test.go:925 annotationPadding 1bcd377f44fe36a5
fn annotation_padding(value: &str) -> String {
    let mut padding = String::new();
    for current in value.chars() {
        if current == '\t' {
            padding.push('\t');
        } else {
            padding.push(' ');
        }
    }
    padding
}

// go test: internal/rules/rules_json_test.go:937 utf16OffsetToByteIndex 6396457afed74b31
fn utf16_offset_to_byte_index(value: &str, offset: i64) -> usize {
    let mut units = 0;
    for (index, current) in value.char_indices() {
        if units >= offset {
            return index;
        }
        units += 1;
        if u32::from(current) > 0xffff {
            units += 1;
        }
    }
    value.len()
}

// go test: internal/rules/rules_json_test.go:951 utf16Length 7fb0d40cbeba3233
fn utf16_length(value: &str) -> i64 {
    let mut units = 0;
    for current in value.chars() {
        units += 1;
        if u32::from(current) > 0xffff {
            units += 1;
        }
    }
    units
}

// go test: internal/rules/rules_json_test.go:962 markdownCodeFence 8d84487835ff4ce7
fn markdown_code_fence(value: &str) -> String {
    let mut longest = 0;
    let mut current = 0;
    for character in value.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    "`".repeat(3.max(longest + 1))
}

/// Rust-only: the `reflect.StructField` facts `generateReadmeExampleConfig` reads of `etscore.EffectPluginOptions`,
/// in field order: the Go field name, the `json` tag, the `schema_description` tag and the `schema_default` tag
/// (`None` without one). Keep in sync with etscore/options.go.
struct StructField {
    name: &'static str,
    json: &'static str,
    schema_description: &'static str,
    schema_default: Option<&'static str>,
}

/// Rust-only: `reflect.VisibleFields(reflect.TypeFor[etscore.EffectPluginOptions]())` (see `StructField`).
static EFFECT_PLUGIN_OPTION_FIELDS: &[StructField] = &[
    StructField {
        name: "Refactors",
        json: "refactors,omitzero",
        schema_description: "Controls Effect refactors.",
        schema_default: Some("true"),
    },
    StructField {
        name: "Diagnostics",
        json: "diagnostics,omitzero",
        schema_description: "Controls Effect diagnostics.",
        schema_default: Some("true"),
    },
    StructField {
        name: "IncludeSuggestionsInTsc",
        json: "includeSuggestionsInTsc,omitzero",
        schema_description: "When false, suggestion-level Effect diagnostics are omitted from tsc CLI output.",
        schema_default: Some("true"),
    },
    StructField {
        name: "Quickinfo",
        json: "quickinfo,omitzero",
        schema_description: "Controls Effect quickinfo.",
        schema_default: Some("true"),
    },
    StructField {
        name: "Completions",
        json: "completions,omitzero",
        schema_description: "Controls Effect completions.",
        schema_default: Some("true"),
    },
    StructField {
        name: "Goto",
        json: "goto,omitzero",
        schema_description: "Controls Effect goto references support.",
        schema_default: Some("true"),
    },
    StructField {
        name: "Renames",
        json: "renames,omitzero",
        schema_description: "Controls Effect rename helpers.",
        schema_default: Some("true"),
    },
    StructField {
        name: "IgnoreEffectSuggestionsInTscExitCode",
        json: "ignoreEffectSuggestionsInTscExitCode,omitzero",
        schema_description: "When true, suggestion diagnostics do not affect the tsc exit code.",
        schema_default: Some("true"),
    },
    StructField {
        name: "IgnoreEffectWarningsInTscExitCode",
        json: "ignoreEffectWarningsInTscExitCode,omitzero",
        schema_description: "When true, warning diagnostics do not affect the tsc exit code.",
        schema_default: Some("false"),
    },
    StructField {
        name: "IgnoreEffectErrorsInTscExitCode",
        json: "ignoreEffectErrorsInTscExitCode,omitzero",
        schema_description: "When true, error diagnostics do not affect the tsc exit code.",
        schema_default: Some("false"),
    },
    StructField {
        name: "SkipDisabledOptimization",
        json: "skipDisabledOptimization,omitzero",
        schema_description: "When true, disabled diagnostics are still processed so directives can re-enable them.",
        schema_default: Some("false"),
    },
    StructField {
        name: "MermaidProvider",
        json: "mermaidProvider,omitzero",
        schema_description: "Mermaid rendering service for layer graph links. Accepts mermaid.live, mermaid.com, or a custom URL.",
        schema_default: Some("\"mermaid.live\""),
    },
    StructField {
        name: "NoExternal",
        json: "noExternal,omitzero",
        schema_description: "When true, suppresses external Mermaid links in hover output.",
        schema_default: Some("false"),
    },
    StructField {
        name: "LayerGraphFollowDepth",
        json: "layerGraphFollowDepth,omitzero",
        schema_description: "How many levels deep the layer graph extraction follows symbol references.",
        schema_default: Some("0"),
    },
    StructField {
        name: "Inlays",
        json: "inlays,omitzero",
        schema_description: "When true, suppresses redundant return-type inlay hints on supported Effect generator functions.",
        schema_default: Some("false"),
    },
    StructField {
        name: "NamespaceImportPackages",
        json: "namespaceImportPackages,omitzero",
        schema_description: "Package names that should prefer namespace imports.",
        schema_default: Some("[]"),
    },
    StructField {
        name: "BarrelImportPackages",
        json: "barrelImportPackages,omitzero",
        schema_description: "Package names that should prefer barrel named imports.",
        schema_default: Some("[]"),
    },
    StructField {
        name: "ImportAliases",
        json: "importAliases,omitzero",
        schema_description: "Package-level import aliases keyed by package name.",
        schema_default: Some("{}"),
    },
    StructField {
        name: "TopLevelNamedReexports",
        json: "topLevelNamedReexports,omitzero",
        schema_description: "Controls whether named reexports are followed at package top-level.",
        schema_default: Some("\"ignore\""),
    },
    StructField {
        name: "KeyPatterns",
        json: "keyPatterns,omitzero",
        schema_description: "Configures key pattern formulas for the deterministicKeys rule.",
        schema_default: None,
    },
    StructField {
        name: "ExtendedKeyDetection",
        json: "extendedKeyDetection,omitzero",
        schema_description: "Enables matching constructors with @effect-identifier annotations.",
        schema_default: Some("false"),
    },
    StructField {
        name: "PipeableMinArgCount",
        json: "pipeableMinArgCount,omitzero",
        schema_description: "Minimum number of contiguous pipeable transformations to trigger missedPipeableOpportunity.",
        schema_default: Some("2"),
    },
    StructField {
        name: "AllowedUnstableApis",
        json: "allowedUnstableApis,omitzero",
        schema_description: "Unstable APIs allowed by package/module subtree or package/module#export name.",
        schema_default: Some("[]"),
    },
    StructField {
        name: "AllowedExperimentalApis",
        json: "allowedExperimentalApis,omitzero",
        schema_description: "Experimental APIs allowed by package/module subtree or package/module#export name.",
        schema_default: Some("[]"),
    },
    StructField {
        name: "AllowedDuplicatedPackages",
        json: "allowedDuplicatedPackages,omitzero",
        schema_description: "Package names allowed to have multiple versions without triggering duplicatePackage.",
        schema_default: Some("[]"),
    },
    StructField {
        name: "EffectFn",
        json: "effectFn,omitzero",
        schema_description: "Controls which effectFnOpportunity quickfix variants are offered.",
        schema_default: Some("[\"span\"]"),
    },
    StructField {
        name: "BrandedSchemaClasses",
        json: "brandedSchemaClasses,omitzero",
        schema_description: "Schema class kinds that get a nominal brand, used by schemaClassMissingBrand and the Schema class completions.",
        schema_default: Some("[\"Class\",\"TaggedClass\",\"Error\",\"TaggedError\",\"Opaque\"]"),
    },
    StructField {
        name: "DiagnosticSeverity",
        json: "diagnosticSeverity,omitzero",
        schema_description: "Maps rule names to severity levels. Use {} to enable diagnostics with rule defaults.",
        schema_default: None,
    },
    StructField {
        name: "Overrides",
        json: "overrides,omitzero",
        schema_description: "Ordered per-file diagnostic option overrides.",
        schema_default: None,
    },
];

// go test: internal/rules/rules_json_test.go:976 generateReadmeExampleConfig 9129dabef0658011
fn generate_readme_example_config() -> String {
    struct ReadmeEntry {
        name: String,
        description: &'static str,
        value: Json,
    }
    let fields = EFFECT_PLUGIN_OPTION_FIELDS;
    let mut entries = Vec::with_capacity(fields.len());
    for field in fields {
        let name = json_field_name(field);
        if name.is_empty() {
            continue;
        }
        let Some(default_value) = readme_default_value(field) else {
            continue;
        };
        entries.push(ReadmeEntry {
            name,
            description: field.schema_description,
            value: default_value,
        });
    }

    let mut lines: Vec<String> = vec![
        "```jsonc".to_string(),
        "{".to_string(),
        r#"  "compilerOptions": {"#.to_string(),
        r#"    "plugins": ["#.to_string(),
        "      {".to_string(),
        r#"        "name": "@effect/language-service","#.to_string(),
    ];
    for (i, entry) in entries.iter().enumerate() {
        let default_text = compact_json(&entry.value);
        if !entry.description.is_empty() {
            lines.push(format!("        // {} (default: {default_text})", entry.description));
        }
        let encoded = indented_json(&entry.value, "        ");
        let comma = if i == entries.len() - 1 { "" } else { "," };
        lines.push(format!("        {}: {encoded}{comma}", ets_go::strconv::quote(&entry.name)));
    }
    lines.push("      }".to_string());
    lines.push("    ]".to_string());
    lines.push("  }".to_string());
    lines.push("}".to_string());
    lines.push("```".to_string());

    lines.join("\n")
}

// go test: internal/rules/rules_json_test.go:1029 readmeDefaultValue 97621bfab4a25a36
fn readme_default_value(field: &StructField) -> Option<Json> {
    if let Some(default_value) = decode_struct_tag_json(field.schema_default) {
        return Some(default_value);
    }
    match field.name {
        "DiagnosticSeverity" => Some(Json::Object(Vec::new())),
        "KeyPatterns" => Some(Json::Array(
            ets_etscore::DEFAULT_KEY_PATTERNS
                .iter()
                .map(|pattern| {
                    // Go's struct encoding: the `json` tags, in field order.
                    Json::Object(vec![
                        ("target".to_string(), Json::String(pattern.target.clone())),
                        ("pattern".to_string(), Json::String(pattern.pattern.clone())),
                        (
                            "skipLeadingPath".to_string(),
                            // Go encodes a nil slice as `null`.
                            pattern.skip_leading_path.as_ref().map_or(Json::Null, |paths| {
                                Json::Array(paths.iter().cloned().map(Json::String).collect())
                            }),
                        ),
                    ])
                })
                .collect(),
        )),
        "Overrides" => Some(Json::Array(vec![Json::object_from_map(vec![
            ("include".to_string(), Json::Array(vec![Json::String("src/**/*.ts".to_string())])),
            (
                "options".to_string(),
                Json::object_from_map(vec![(
                    "diagnosticSeverity".to_string(),
                    Json::object_from_map(vec![(
                        "floatingEffect".to_string(),
                        Json::String("error".to_string()),
                    )]),
                )]),
            ),
        ])])),
        _ => None,
    }
}

// go test: internal/rules/rules_json_test.go:1054 decodeStructTagJSON 13723c83eebd963e
//
// Rust: takes the tag value (`field.Tag.Get(key)`, `None` for an absent tag) instead of the field and key.
fn decode_struct_tag_json(value: Option<&str>) -> Option<Json> {
    let value = value.filter(|value| !value.is_empty())?;
    let decoded: serde_json::Value = match serde_json::from_str(value) {
        Ok(decoded) => decoded,
        Err(err) => panic!("{err}"),
    };
    Some(Json::from_decoded(&decoded))
}

// go test: internal/rules/rules_json_test.go:1066 jsonFieldName 632c44eff9e2e6a9
fn json_field_name(field: &StructField) -> String {
    let tag = field.json;
    if tag.is_empty() {
        return String::new();
    }
    let name = tag.split(',').next().unwrap_or_default();
    if name == "-" {
        return String::new();
    }
    name.to_string()
}

// go test: internal/rules/rules_json_test.go:1078 compactJSON bbfc53f64c0f0924
fn compact_json(value: &Json) -> String {
    value.marshal()
}

// go test: internal/rules/rules_json_test.go:1086 indentedJSON 0d0e46983e7852dc
fn indented_json(value: &Json, prefix: &str) -> String {
    let data = value.marshal_indent("", "  ");
    let mut parts: Vec<String> = data.split('\n').map(str::to_string).collect();
    if parts.len() == 1 {
        return parts.swap_remove(0);
    }
    for part in parts.iter_mut().skip(1) {
        *part = format!("{prefix}{part}");
    }
    parts.join("\n")
}

// go test: internal/rules/rules_json_test.go:1101 readmeStartMarker 2794346749d2fb40
const README_START_MARKER: &str = "<!-- diagnostics-table:start -->";
const README_END_MARKER: &str = "<!-- diagnostics-table:end -->";
const README_EXAMPLE_START_MARKER: &str = "<!-- example-config:start -->";
const README_EXAMPLE_END_MARKER: &str = "<!-- example-config:end -->";

// go test: internal/rules/rules_json_test.go:1108 generateReadme cf02671f61c5fa66
fn generate_readme(committed_readme: &[u8]) -> Result<Vec<u8>, String> {
    let content = String::from_utf8_lossy(committed_readme).into_owned();
    let diagnostics_start_idx = content.find(README_START_MARKER);
    let diagnostics_end_idx = content.find(README_END_MARKER);
    match (diagnostics_start_idx, diagnostics_end_idx) {
        (Some(start), Some(end)) if end > start => {}
        _ => return Err("README.md missing diagnostics table markers".to_string()),
    }
    let example_start_idx = content.find(README_EXAMPLE_START_MARKER);
    let example_end_idx = content.find(README_EXAMPLE_END_MARKER);
    match (example_start_idx, example_end_idx) {
        (Some(start), Some(end)) if end > start => {}
        _ => return Err("README.md missing example config markers".to_string()),
    }

    let table = generate_readme_table();
    let example = generate_readme_example_config();
    let content = replace_readme_section(&content, README_START_MARKER, README_END_MARKER, &table);
    let content = replace_readme_section(
        &content,
        README_EXAMPLE_START_MARKER,
        README_EXAMPLE_END_MARKER,
        &example,
    );

    Ok(content.into_bytes())
}

// go test: internal/rules/rules_json_test.go:1129 replaceReadmeSection 826bd5cc6982bbe8
fn replace_readme_section(
    content: &str,
    start_marker: &str,
    end_marker: &str,
    body: &str,
) -> String {
    let Some((before, after_start)) = content.split_once(start_marker) else {
        panic!("missing start marker: {start_marker}");
    };
    let Some((_, after_end)) = after_start.split_once(end_marker) else {
        panic!("missing end marker: {end_marker}");
    };
    let mut buf = String::new();
    buf.push_str(before);
    buf.push_str(start_marker);
    buf.push('\n');
    buf.push_str(body);
    buf.push('\n');
    buf.push_str(end_marker);
    buf.push_str(after_end);
    buf
}

/// Rust-only: Go's `fmt.Fprintf(&builder, ...)` on a `strings.Builder`.
fn push_fmt(out: &mut String, args: std::fmt::Arguments<'_>) {
    std::fmt::Write::write_fmt(out, args).expect("writing to a String cannot fail");
}

/// Rust-only: Go's `html.EscapeString`.
fn html_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&#34;"),
            _ => out.push(c),
        }
    }
    out
}

/// Rust-only: a JSON value encoded as Go's `encoding/json` encodes the test's values: objects keep their field order
/// (Go structs) or are sorted by key when built from a map (`object_from_map`, Go maps), strings are escaped with
/// Go's HTML-safe escaping, numbers are pre-formatted.
#[derive(Clone, Debug)]
enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A Go map: encoded with its keys sorted.
    fn object_from_map(entries: impl IntoIterator<Item = (String, Json)>) -> Json {
        let mut entries: Vec<(String, Json)> = entries.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Json::Object(entries)
    }

    /// Go's `json.Unmarshal` into `any` (objects become maps, numbers float64).
    fn from_decoded(value: &serde_json::Value) -> Json {
        match value {
            serde_json::Value::Null => Json::Null,
            serde_json::Value::Bool(b) => Json::Bool(*b),
            serde_json::Value::Number(n) => {
                Json::Number(go_json_float(n.as_f64().unwrap_or_default()))
            }
            serde_json::Value::String(s) => Json::String(s.clone()),
            serde_json::Value::Array(items) => {
                Json::Array(items.iter().map(Json::from_decoded).collect())
            }
            serde_json::Value::Object(fields) => Json::object_from_map(
                fields.iter().map(|(k, v)| (k.clone(), Json::from_decoded(v))),
            ),
        }
    }

    /// `json.Marshal`.
    fn marshal(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None, 0);
        out
    }

    /// `json.MarshalIndent(v, prefix, indent)`.
    fn marshal_indent(&self, prefix: &str, indent: &str) -> String {
        let mut out = String::new();
        self.write(&mut out, Some((prefix, indent)), 0);
        out
    }

    fn write(&self, out: &mut String, indent: Option<(&str, &str)>, depth: usize) {
        let newline = |out: &mut String, depth: usize| {
            if let Some((prefix, indent)) = indent {
                out.push('\n');
                out.push_str(prefix);
                for _ in 0..depth {
                    out.push_str(indent);
                }
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => out.push_str(n),
            Json::String(s) => write_go_json_string(out, s),
            Json::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, depth + 1);
                    item.write(out, indent, depth + 1);
                }
                newline(out, depth);
                out.push(']');
            }
            Json::Object(fields) => {
                if fields.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, depth + 1);
                    write_go_json_string(out, key);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    value.write(out, indent, depth + 1);
                }
                newline(out, depth);
                out.push('}');
            }
        }
    }
}

/// Rust-only: `encoding/json`'s string encoding with HTML escaping (`json.Marshal`'s default).
fn write_go_json_string(out: &mut String, s: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => {
                let code = u32::from(c);
                out.push_str("\\u");
                for shift in [12, 8, 4, 0] {
                    out.push(char::from(HEX[((code >> shift) & 0xf) as usize]));
                }
            }
            c if u32::from(c) < 0x20 => {
                let code = u32::from(c);
                out.push_str("\\u00");
                out.push(char::from(HEX[(code >> 4) as usize]));
                out.push(char::from(HEX[(code & 0xf) as usize]));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Rust-only: `encoding/json`'s float64 encoding (`floatEncoder`): `'f'` formatting between 1e-6 and 1e21, `'e'`
/// otherwise with a two-digit exponent cleaned up (`e-07` -> `e-7`).
fn go_json_float(f: f64) -> String {
    let abs = f.abs();
    let mut fmt = b'f';
    if abs != 0.0 && !(1e-6..1e21).contains(&abs) {
        fmt = b'e';
    }
    let mut s = ets_go::strconv::format_float(f, fmt, -1, 64);
    if fmt == b'e' {
        let n = s.len();
        if n >= 4
            && s.as_bytes()[n - 4] == b'e'
            && s.as_bytes()[n - 3] == b'-'
            && s.as_bytes()[n - 2] == b'0'
        {
            s.remove(n - 2);
        }
    }
    s
}

impl PreviewDiagnostic {
    fn to_json(&self) -> Json {
        Json::Object(vec![
            ("start".to_string(), Json::Number(self.start.to_string())),
            ("end".to_string(), Json::Number(self.end.to_string())),
            ("text".to_string(), Json::String(self.text.clone())),
        ])
    }
}

impl PreviewPayload {
    fn to_json(&self) -> Json {
        Json::Object(vec![
            ("sourceText".to_string(), Json::String(self.source_text.clone())),
            (
                "diagnostics".to_string(),
                Json::Array(self.diagnostics.iter().map(PreviewDiagnostic::to_json).collect()),
            ),
        ])
    }
}

impl ExportedRule {
    fn to_json(&self) -> Json {
        let mut fields = vec![
            ("name".to_string(), Json::String(self.name.clone())),
            ("group".to_string(), Json::String(self.group.clone())),
            ("description".to_string(), Json::String(self.description.clone())),
            (
                "defaultSeverity".to_string(),
                Json::String(self.default_severity.string().to_string()),
            ),
            ("fixable".to_string(), Json::Bool(self.fixable)),
            (
                "supportedEffect".to_string(),
                Json::Array(self.supported_effect.iter().cloned().map(Json::String).collect()),
            ),
            (
                "codes".to_string(),
                Json::Array(self.codes.iter().map(|code| Json::Number(code.to_string())).collect()),
            ),
        ];
        if !self.replaced_by.is_empty() {
            fields.push((
                "replacedBy".to_string(),
                Json::Array(self.replaced_by.iter().cloned().map(Json::String).collect()),
            ));
        }
        if let Some(preview) = &self.preview {
            fields.push(("preview".to_string(), preview.to_json()));
        }
        Json::Object(fields)
    }
}

impl MetadataDocument {
    /// Go's `json.Marshal` of the document (`rule.MetadataGroup` / `MetadataPreset` by their `json` tags).
    fn to_json(&self) -> Json {
        let groups = self
            .groups
            .iter()
            .map(|g| {
                Json::Object(vec![
                    ("id".to_string(), Json::String(g.id.to_string())),
                    ("name".to_string(), Json::String(g.name.to_string())),
                    ("description".to_string(), Json::String(g.description.to_string())),
                ])
            })
            .collect();
        let presets = self
            .presets
            .iter()
            .map(|p| {
                Json::Object(vec![
                    ("name".to_string(), Json::String(p.name.to_string())),
                    ("description".to_string(), Json::String(p.description.to_string())),
                    (
                        "diagnosticSeverity".to_string(),
                        Json::object_from_map(
                            p.diagnostic_severity
                                .iter()
                                .map(|(k, v)| (k.clone(), Json::String(v.string().to_string()))),
                        ),
                    ),
                ])
            })
            .collect();
        Json::Object(vec![
            ("groups".to_string(), Json::Array(groups)),
            ("presets".to_string(), Json::Array(presets)),
            (
                "rules".to_string(),
                Json::Array(self.rules.iter().map(ExportedRule::to_json).collect()),
            ),
        ])
    }

    /// Go's `json.Unmarshal` into a `metadataDocument` (missing fields keep their zero values).
    fn unmarshal(data: &str) -> Result<MetadataDocument, String> {
        fn leak(s: &str) -> &'static str {
            Box::leak(s.to_string().into_boxed_str())
        }
        fn str_field(value: &serde_json::Value, key: &str) -> String {
            value.get(key).and_then(serde_json::Value::as_str).unwrap_or_default().to_string()
        }
        fn int_field(value: &serde_json::Value, key: &str) -> i64 {
            value.get(key).and_then(serde_json::Value::as_i64).unwrap_or_default()
        }
        fn array<'v>(value: &'v serde_json::Value, key: &str) -> &'v [serde_json::Value] {
            value
                .get(key)
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
        }
        let value: serde_json::Value = serde_json::from_str(data).map_err(|err| err.to_string())?;
        let groups = array(&value, "groups")
            .iter()
            .map(|g| MetadataGroup {
                id: leak(&str_field(g, "id")),
                name: leak(&str_field(g, "name")),
                description: leak(&str_field(g, "description")),
            })
            .collect();
        let presets = array(&value, "presets")
            .iter()
            .map(|p| {
                let mut diagnostic_severity = FxHashMap::default();
                if let Some(map) =
                    p.get("diagnosticSeverity").and_then(serde_json::Value::as_object)
                {
                    for (k, v) in map {
                        diagnostic_severity.insert(
                            k.clone(),
                            ets_etscore::parse_severity(v.as_str().unwrap_or_default()),
                        );
                    }
                }
                MetadataPreset {
                    name: leak(&str_field(p, "name")),
                    description: leak(&str_field(p, "description")),
                    diagnostic_severity,
                }
            })
            .collect();
        let rules = array(&value, "rules")
            .iter()
            .map(|r| ExportedRule {
                name: str_field(r, "name"),
                group: str_field(r, "group"),
                description: str_field(r, "description"),
                default_severity: ets_etscore::parse_severity(&str_field(r, "defaultSeverity")),
                fixable: r.get("fixable").and_then(serde_json::Value::as_bool).unwrap_or_default(),
                supported_effect: array(r, "supportedEffect")
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect(),
                codes: array(r, "codes")
                    .iter()
                    .map(|c| c.as_i64().unwrap_or_default() as i32)
                    .collect(),
                replaced_by: array(r, "replacedBy")
                    .iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect(),
                preview: r.get("preview").filter(|p| !p.is_null()).map(|p| PreviewPayload {
                    source_text: str_field(p, "sourceText"),
                    diagnostics: array(p, "diagnostics")
                        .iter()
                        .map(|d| PreviewDiagnostic {
                            start: int_field(d, "start"),
                            end: int_field(d, "end"),
                            text: str_field(d, "text"),
                        })
                        .collect(),
                }),
            })
            .collect();
        Ok(MetadataDocument { groups, presets, rules })
    }
}
