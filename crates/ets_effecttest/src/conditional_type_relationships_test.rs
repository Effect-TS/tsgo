use std::sync::Arc;

use ets_bundledeffect::EffectVersion;
use ets_go::testing::{T, on_checker_stack};
use ets_shim::compiler::{CompilerHost, ProgramOptions, ProgramRegions};
use ets_shim::core::P;
use ets_shim::core::arena::Region;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::{self, ComparePathsOptions};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};

use crate::*;

// Go's //go:embed reads this same fixture.
const CONDITIONAL_TYPE_RELATIONSHIPS_SOURCE: &str =
    include_str!("../../../internal/effecttest/testdata/conditionalTypeRelationships.ts");

// go test: internal/effecttest/conditional_type_relationships_test.go TestConditionalTypeRelationships
#[test]
fn test_conditional_type_relationships() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();
            for mode in ["plugin-off", "rule-off", "enabled"] {
                t.run(mode, |t| {
                    t.parallel();
                    let severity = if mode == "enabled" { "error" } else { "off" };
                    let source = CONDITIONAL_TYPE_RELATIONSHIPS_SOURCE
                        .replace("effectInFailure:CONTROL", &format!("effectInFailure:{severity}"));
                    let units = parse_test_units(&source, "conditionalTypeRelationships.ts");
                    let mut want = vec![
                        conditional_relationship_expected(
                            t, &units, "returning.d.ts", 2430, "BrokenReturning extends", "BrokenReturning",
                        ),
                        conditional_relationship_expected(
                            t, &units, "invalid.ts", 2322, "badAssignment: number", "badAssignment",
                        ),
                        conditional_relationship_expected(
                            t, &units, "invalid.ts", 2322, "badNarrowed: T extends", "badNarrowed",
                        ),
                    ];
                    if mode == "enabled" {
                        want.push(conditional_relationship_expected(
                            t, &units, "effectControl.ts", 377054, "Effect.try({",
                            "Effect.try({\n  try: () => 1,\n  catch: () => Effect.logError(\"boom\"),\n})",
                        ));
                    }
                    let (mut got, details) =
                        collect_conditional_relationship_diagnostics(t, &units, mode != "plugin-off");
                    got.sort();
                    want.sort();
                    if got != want {
                        t.fatal(format!(
                            "diagnostic locations = {got:?}, want {want:?}\n{}",
                            details.join("\n")
                        ));
                    }
                });
            }
        });
    });
}

// Go: conditionalRelationshipExpected. The expected spans are all ASCII.
fn conditional_relationship_expected(
    t: &T,
    units: &[TestUnit],
    file: &str,
    code: i32,
    anchor: &str,
    span: &str,
) -> String {
    for unit in units {
        if unit.name != file {
            continue;
        }
        if unit.content.matches(anchor).count() != 1 {
            t.fatal(format!("{file}: expected one occurrence of {anchor:?}"));
        }
        let pos = unit.content.find(anchor).unwrap();
        if !unit.content[pos..].starts_with(span) {
            t.fatal(format!("{file}: expected span {span:?} at {anchor:?}"));
        }
        let prefix = &unit.content[..pos];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
        let column = prefix.rfind('\n').map_or(pos + 1, |last| pos - last);
        return format!("/.src/{file}({line},{column},{}): TS{code}", span.len());
    }
    t.fatal(format!("missing fixture unit {file}"));
}

// Go: collectConditionalRelationshipDiagnostics. Unlike the older string-only
// helper, check declarations and default libraries and keep each diagnostic span.
fn collect_conditional_relationship_diagnostics(
    t: &T,
    units: &[TestUnit],
    plugin_enabled: bool,
) -> (Vec<String>, Vec<String>) {
    init_hooks();
    acquire_program();
    let _release = ReleaseProgramOnDrop;

    let current_directory = "/.src";
    let mut testfs = ets_bundledeffect::TestFS::default();
    if let Err(err) = ets_bundledeffect::mount_effect(EffectVersion::V4, &mut testfs) {
        t.fatal(err.to_string());
    }
    testfs
        .insert(format!("{current_directory}/package.json"), MapFile::from(r#"{"type":"module"}"#));

    let region = Region::new_scratch(1 << 20);
    let region_scope = region.enter();
    let mut file_names = Vec::new();
    for unit in units {
        let path = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        testfs.insert(path.clone(), MapFile::from(unit.content.as_str()));
        file_names.push(path);
    }
    let plugins = if plugin_enabled { r#"[{"name":"@effect/language-service"}]"# } else { "[]" };
    let config_text = format!(
        r#"{{"compilerOptions":{{
            "target":"ESNext","module":"NodeNext","moduleResolution":"NodeNext",
            "strict":true,"noEmit":true,"noErrorTruncation":true,
            "skipLibCheck":false,"skipDefaultLibCheck":false,"plugins":{plugins}
        }}}}"#
    );
    let config_name = format!("{current_directory}/tsconfig.json");
    testfs.insert(config_name.clone(), MapFile::from(config_text.as_str()));
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(testfs, true)));
    let config_file = parse_ts_config_source_file(&config_name, &config_text, current_directory);
    let config_host = DefaultParseConfigHost {
        fs: Arc::clone(&fs),
        current_directory: current_directory.to_string(),
    };
    // SAFETY (lifetime only): config parsing reads but does not retain this host,
    // as in collect_diagnostic_strings_from_content.
    let config_host = unsafe { assume_static(&config_host) };
    let config = ets_shim::tsoptions::parse_json_source_file_config_file_content(
        config_file,
        config_host,
        current_directory,
        None,
        None,
        &config_name,
        &[],
        None,
    );
    let mut program_config = ets_shim::tsoptions::new_parsed_command_line(
        config.compiler_options().unwrap(),
        file_names,
        Vec::new(),
        ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: current_directory.to_string(),
        },
    );
    program_config.config_file = config.config_file;
    program_config.errors = config.errors;
    let host: Arc<dyn CompilerHost> = Arc::new(CachingCompilerHost {
        compiler_host: ets_shim::compiler::new_compiler_host(
            current_directory,
            fs,
            &bundled::lib_path(),
            None,
            None,
            None,
        ),
        version: EffectVersion::V4,
    });
    let mut program_options = ProgramOptions::new(P::new(program_config), host);
    program_options.single_threaded = ets_shim::core::Tristate::True;
    program_options.checker_regions = true;
    program_options.checker_diagnostic_regions = true;
    let loader_regions = Arc::new(ProgramRegions::default());
    program_options.regions = Some(Arc::clone(&loader_regions));
    let program = ets_shim::compiler::new_program(program_options);

    let ctx = Context::background();
    let mut diagnostics = Vec::new();
    diagnostics.extend(program.get_config_file_parsing_diagnostics());
    diagnostics.extend(program.get_program_diagnostics());
    diagnostics.extend(program.get_syntactic_diagnostics(&ctx, None));
    diagnostics.extend(program.get_semantic_diagnostics(&ctx, None));
    diagnostics.extend(program.get_global_diagnostics(&ctx));
    let mut locations = Vec::new();
    let mut details = Vec::new();
    for diag in &diagnostics {
        let mut location = "<global>".to_string();
        if let (Some(file), Some(pos), Some(end)) = (diag.file(), diag.pos(), diag.end()) {
            let (line, column) =
                ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(&*file, pos);
            location = format!("{}({},{},{})", file.file_name(), line + 1, column + 1, end - pos);
        }
        let location = format!("{location}: TS{}", diag.code());
        details.push(format!("{location}: {}", diag.localize()));
        locations.push(location);
    }
    drop(diagnostics);
    drop(region_scope);
    // SAFETY: only owned strings escape; all program-backed pointers are unused.
    unsafe { release_test_program(program, loader_regions, region) };
    (locations, details)
}
