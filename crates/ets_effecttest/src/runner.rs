use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::ast::{SourceFile, SourceFileParseOptions};
use ets_shim::compiler::{CompilerHost, ProgramOptions, ProgramRegions};
use ets_shim::core::arena::Region;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::{self, ComparePathsOptions};
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, NewLineKind, P, ScriptKind, ScriptTarget,
    Tristate,
};
use ets_shim::tsoptions::{ParsedCommandLine, TsConfigSourceFile};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use regex::Regex;

use crate::sharedfs::{get_shared_packages, new_overlay_fs};
use crate::*;

// go test: internal/effecttest/runner.go:30 TestCasesDir fc67156a98ca53d0
// TestCasesDir returns the path to the Effect test cases directory for the given version.
pub fn test_cases_dir(version: EffectVersion) -> PathBuf {
    ets_bundledeffect::effect_ts_go_root_path()
        .join("testdata")
        .join("tests")
        .join(version.as_str())
}

// go test: internal/effecttest/runner.go:35 DiscoverTestCases 7bcb60f789001d56
// DiscoverTestCases finds all .ts test files in the test cases directory for the given version.
pub fn discover_test_cases(version: EffectVersion) -> std::io::Result<Vec<PathBuf>> {
    let dir = test_cases_dir(version);
    let entries = read_dir_sorted(&dir)?;

    let mut cases = Vec::new();
    for (name, is_dir) in entries {
        if !is_dir && Path::new(&name).extension().is_some_and(|e| e == "ts") {
            cases.push(dir.join(name));
        }
    }
    Ok(cases)
}

/// Rust-only: Go's `os.ReadDir`, which returns the entries sorted by file name: `(name, is directory)`.
pub(crate) fn read_dir_sorted(dir: &Path) -> std::io::Result<Vec<(String, bool)>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let is_dir = entry.file_type()?.is_dir();
        entries.push((entry.file_name().to_string_lossy().into_owned(), is_dir));
    }
    entries.sort();
    Ok(entries)
}

// go test: internal/effecttest/runner.go:51 lineDelimiter c62222199296711a
// Rust: Go's RE2 `\s` and `\w` are ASCII classes; they are spelled out, as the regex crate's are Unicode.
pub(crate) static LINE_DELIMITER: LazyLock<Regex> = LazyLock::new(|| Regex::new("\r?\n").unwrap());
// go test: internal/effecttest/runner.go:51 lineDelimiter c62222199296711a
pub(crate) static OPTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^/{2}[\t\n\f\r ]*@([0-9A-Za-z_]+)[\t\n\f\r ]*:[\t\n\f\r ]*([^\r\n]*)").unwrap()
});

// go test: internal/effecttest/runner.go:57 testUnit eca245d18458a581
// testUnit represents a single file within a multi-file test case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TestUnit {
    pub(crate) name: String,
    pub(crate) content: String,
}

// go test: internal/effecttest/runner.go:63 parseTestUnits 3387216c27ac2947
// parseTestUnits parses a test file into multiple test units based on @filename directives.
pub(crate) fn parse_test_units(content: &str, default_file_name: &str) -> Vec<TestUnit> {
    let lines = LINE_DELIMITER.split(content);

    let mut units = Vec::new();
    let mut current_content = String::new();
    let mut current_file_name = String::new();

    for line in lines {
        if let Some(test_meta_data) = OPTION_REGEX.captures(line) {
            let meta_data_name = test_meta_data[1].to_ascii_lowercase();
            if meta_data_name == "filename" {
                // Save the current file if we have one
                if !current_file_name.is_empty() && !current_content.is_empty() {
                    units.push(TestUnit {
                        name: current_file_name.clone(),
                        content: current_content.clone(),
                    });
                }
                // Start new file
                current_file_name = test_meta_data[2].trim().to_string();
                current_content.clear();
                continue;
            }
        }
        // Add line to current file content
        if !current_content.is_empty() {
            current_content.push('\n');
        }
        current_content.push_str(line);
    }

    // Handle the final file
    if !current_file_name.is_empty() {
        units.push(TestUnit { name: current_file_name, content: current_content });
    } else if !current_content.is_empty() {
        // Single file test
        units.push(TestUnit { name: default_file_name.to_string(), content: current_content });
    }

    units
}

// go test: internal/effecttest/runner.go:114 DefaultTsConfig b7d3467b28ac3b3b
// DefaultTsConfig is the default tsconfig content injected when a test does not provide one.
// It enables the Effect language service plugin while keeping stability rules
// off in unrelated fixtures. Dedicated fixtures enable them explicitly.
pub const DEFAULT_TS_CONFIG: &str = r#"{
  "compilerOptions": {
    "skipLibCheck": true,
    "plugins": [
      {
        "name": "@effect/language-service",
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true,
        "diagnosticSeverity": {
          "experimentalApiUsage": "off",
          "unstableApiUsage": "off",
          "apiStabilityLeak": "off"
        }
      }
    ]
  }
}"#;

// go test: internal/effecttest/runner.go:133 RunEffectTest 44d084c777c0dd1a
// RunEffectTest executes a single Effect diagnostic test case for the given version.
//
// Rust: the Effect diagnostics come from the hook crates `init_hooks` registers (`etscheckerhooks`). Go's
// `harnessutil.NewOutputRecorderFS` is left out: it records emitted files and this runner emits nothing.
// Freeing the program at the end is Rust-only (Go leaves it to the GC).
pub fn run_effect_test(t: &T, version: EffectVersion, test_file: &Path) {
    init_hooks();
    acquire_program();
    let _release = ReleaseProgramOnDrop;

    // Read the test file
    let content = match std::fs::read_to_string(test_file) {
        Ok(content) => content,
        Err(err) => t.fatal(format!("Failed to read test file: {err}")),
    };

    // Parse test file into units (handles @filename directives)
    let default_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let units = parse_test_units(&content, &default_file_name);

    // Create the test filesystem
    let mut testfs = ets_bundledeffect::TestFS::default();

    // Effect (and Vitest) packages come from a VFS shared by all tests
    let shared = match get_shared_packages(version, content.contains("vitest")) {
        Ok(shared) => shared,
        Err(err) => t.fatal(format!("Failed to mount Effect: {err}")),
    };
    testfs.insert("/.src/package.json".to_string(), MapFile::from(shared.package_json.as_slice()));

    // Current directory for the test
    let current_directory = "/.src";

    // Rust-only: everything the case allocates outside the program (the parsed tsconfig, the command lines) goes to
    // this region, freed with the program at the end (Go: the GC).
    let region = Region::new_scratch(1 << 20);
    let region_scope = region.enter();

    // Add test files to VFS
    let mut input_files: Vec<TestFile> = Vec::new();
    let mut ts_config_file: Option<P<TsConfigSourceFile>> = None;
    let mut ts_config_unit: Option<TestUnit> = None;
    let mut tsconfig_injected = false;

    for unit in &units {
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        testfs.insert(unit_name.clone(), MapFile::from(unit.content.as_str()));

        // Files placed under /node_modules/ via @filename directives are
        // VFS-only: they set up synthetic packages but are not added to the
        // program's root file list.  The compiler discovers them through
        // module resolution when test source files import from them.
        if unit_name.starts_with("/node_modules/") {
            continue;
        }

        // Check if this is a tsconfig.json file
        if !get_config_name_from_file_name(&unit.name).is_empty() {
            // Parse tsconfig
            ts_config_file =
                Some(parse_ts_config_source_file(&unit_name, &unit.content, current_directory));
            ts_config_unit = Some(unit.clone());
        } else {
            input_files.push(TestFile { unit_name, content: unit.content.clone() });
        }
    }

    // Inject default tsconfig if none was provided by the test
    if ts_config_unit.is_none() {
        tsconfig_injected = true;
        let unit =
            TestUnit { name: "tsconfig.json".to_string(), content: DEFAULT_TS_CONFIG.to_string() };
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        testfs.insert(unit_name.clone(), MapFile::from(unit.content.as_str()));
        ts_config_file =
            Some(parse_ts_config_source_file(&unit_name, &unit.content, current_directory));
        ts_config_unit = Some(unit);
    }

    // Create VFS
    let fs = new_overlay_fs(vfstest::from_map(testfs, true), shared);
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(fs));

    // Setup compiler options
    let mut compiler_options = P::new(CompilerOptions {
        new_line: NewLineKind::LF,
        skip_default_lib_check: Tristate::True,
        skip_lib_check: Tristate::True,
        no_error_truncation: Tristate::True,
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        // Enable esModuleInterop for Effect
        es_module_interop: Tristate::True,
        allow_synthetic_default_imports: Tristate::True,
        ..CompilerOptions::default()
    });

    let mut parsed_config: Option<ParsedCommandLine> = None;

    // Parse tsconfig if present
    if let (Some(ts_config_file), Some(ts_config_unit)) = (ts_config_file, &ts_config_unit) {
        let config_dir = tspath::get_directory_path(&ts_config_unit.name);
        let config_dir = tspath::get_normalized_absolute_path(&config_dir, current_directory);

        let host = DefaultParseConfigHost {
            fs: Arc::clone(&fs),
            current_directory: current_directory.to_string(),
        };
        let config_file_name = ts_config_file.source_file.file_name().to_string();
        // SAFETY (lifetime only): tsoptions reads the host during the call and keeps nothing of it (TSRS_API.md,
        // "Gaps": `&'static dyn ParseConfigHost` for a non-static host).
        let host: &'static DefaultParseConfigHost = unsafe { assume_static(&host) };
        let parsed = ets_shim::tsoptions::parse_json_source_file_config_file_content(
            ts_config_file,
            host,
            &config_dir,
            None,
            None,
            &config_file_name,
            &[],
            None,
        );
        // Use parsed compiler options
        if let Some(options) = parsed.compiler_options() {
            // Merge with our defaults
            let mut options = (*options).clone();
            options.new_line = NewLineKind::LF;
            options.skip_default_lib_check = Tristate::True;
            options.skip_lib_check = Tristate::True;
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
        parsed_config = Some(parsed);
    }

    // Get file names for compilation
    let program_file_names: Vec<String> =
        input_files.iter().map(|file| file.unit_name.clone()).collect();

    // Create compiler host with AST caching for package files
    let host: Arc<dyn CompilerHost> = Arc::new(CachingCompilerHost {
        compiler_host: ets_shim::compiler::new_compiler_host(
            current_directory,
            fs,
            &bundled::lib_path(),
            None,
            None,
            None,
        ),
        version,
    });

    // Create program
    let config_file = parsed_config.as_ref().and_then(|parsed| parsed.config_file);
    let mut program_config = ets_shim::tsoptions::new_parsed_command_line(
        compiler_options,
        program_file_names,
        Vec::new(),
        ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: current_directory.to_string(),
        },
    );
    program_config.config_file = config_file;
    let mut program_options = ProgramOptions::new(P::new(program_config), host);
    program_options.single_threaded = Tristate::True;
    // Rust-only: the program's checkers, their diagnostics and what its file loader allocates go to regions that
    // `release_test_program` frees (TSRS_API.md, "The API path").
    program_options.checker_regions = true;
    program_options.checker_diagnostic_regions = true;
    let loader_regions = Arc::new(ProgramRegions::default());
    program_options.regions = Some(Arc::clone(&loader_regions));
    let program = ets_shim::compiler::new_program(program_options);

    // Get diagnostics
    let ctx = Context::background();
    let mut diagnostics = Vec::new();
    diagnostics.extend(program.get_program_diagnostics());
    diagnostics.extend(program.get_syntactic_diagnostics(&ctx, None));
    diagnostics.extend(program.get_semantic_diagnostics(&ctx, None));
    diagnostics.extend(program.get_global_diagnostics(&ctx));

    // Prepare files for baseline
    let mut all_files: Vec<TestFile> = Vec::new();
    if let Some(ts_config_unit) = &ts_config_unit {
        if !tsconfig_injected {
            all_files.push(TestFile {
                unit_name: tspath::get_normalized_absolute_path(
                    &ts_config_unit.name,
                    current_directory,
                ),
                content: ts_config_unit.content.clone(),
            });
        }
    }
    all_files.extend(input_files.iter().cloned());

    // Generate error baseline
    let base_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let baseline_name = base_file_name.strip_suffix(".ts").unwrap_or(&base_file_name).to_string();
    let baseline_subfolder = version.as_str();

    t.run("errors", |t| {
        let mut c = program.get_type_checker(&ctx);
        let effect_version =
            ets_typeparser::new_type_parser(program, &mut c).detect_effect_version_string();
        do_effect_error_baseline(
            t,
            &baseline_name,
            &all_files,
            &diagnostics,
            false, // pretty
            baseline_subfolder,
            &effect_version,
        );
    });

    // Generate piping flow baseline
    t.run("pipings", |t| {
        let mut c = program.get_type_checker(&ctx);
        do_piping_flow_baseline(
            t,
            &baseline_name,
            program,
            &mut c,
            &input_files,
            |file_name: &str| program.get_source_file(file_name),
            baseline_subfolder,
        );
    });

    // Generate layer graph baseline
    t.run("layers", |t| {
        let mut c = program.get_type_checker(&ctx);
        do_layer_graph_baseline(
            t,
            &baseline_name,
            program,
            &mut c,
            &input_files,
            |file_name: &str| program.get_source_file(file_name),
            baseline_subfolder,
        );
    });

    drop(diagnostics);
    drop(region_scope);
    // SAFETY: every checker handle and everything read from the program is gone (the baselines are strings).
    unsafe { release_test_program(program, loader_regions, region) };
}

/// Rust-only: Go's `parser.ParseSourceFile(ast.NewSourceFileParseOptions(unitName, path), content, core.ScriptKindJSON)`
/// wrapped in a `tsoptions.TsConfigSourceFile`, as `RunEffectTest` does for a tsconfig unit (twice).
pub(crate) fn parse_ts_config_source_file(
    unit_name: &str,
    content: &str,
    current_directory: &str,
) -> P<TsConfigSourceFile> {
    let path = tspath::to_path(unit_name, current_directory, true);
    let config_json: P<SourceFile> = ets_shim::parser::parse_source_file(
        SourceFileParseOptions { file_name: unit_name.to_string(), path, ..Default::default() },
        content,
        ScriptKind::JSON,
    );
    TsConfigSourceFile::new(config_json)
}
