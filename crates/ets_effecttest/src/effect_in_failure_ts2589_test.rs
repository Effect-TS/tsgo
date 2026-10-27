use std::fmt::Write as _;
use std::sync::Arc;

use ets_bundledeffect::EffectVersion;
use ets_go::testing::{T, on_checker_stack};
use ets_shim::compiler::{CompilerHost, ProgramOptions, ProgramRegions};
use ets_shim::core::arena::Region;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::{self, ComparePathsOptions};
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, NewLineKind, P, ScriptTarget, Tristate,
};
use ets_shim::tsoptions::{ParsedCommandLine, TsConfigSourceFile};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};

use crate::*;

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:22 TestEffectInFailureCanTriggerPluginOnlyTS2589 fb587c1169728509
#[test]
fn test_effect_in_failure_can_trigger_plugin_only_ts2589() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            let with_rule = collect_diagnostic_strings_from_content(
                t,
                &build_effect_in_failure_ts2589_case("enabled"),
            );
            let without_rule = collect_diagnostic_strings_from_content(
                t,
                &build_effect_in_failure_ts2589_case("rule-off"),
            );
            let without_plugin = collect_diagnostic_strings_from_content(
                t,
                &build_effect_in_failure_ts2589_case("plugin-off"),
            );

            if !has_diagnostic_code(&with_rule, "TS2589:") {
                t.fatal(format!("expected TS2589 with effectInFailure enabled, got {with_rule:?}"));
            }
            if !has_diagnostic_code(&with_rule, "TS377054:") {
                t.fatal(format!(
                    "expected effectInFailure diagnostic with rule enabled, got {with_rule:?}"
                ));
            }
            if has_diagnostic_code(&without_rule, "TS2589:") {
                t.fatal(format!(
                    "did not expect TS2589 with effectInFailure disabled, got {without_rule:?}"
                ));
            }
            if has_diagnostic_code(&without_plugin, "TS2589:") {
                t.fatal(format!(
                    "did not expect TS2589 with plugin disabled, got {without_plugin:?}"
                ));
            }
        });
    });
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:43 TestTaggedTemplateSymbolInterpolationDoesNotReportTS2731 6f9e8c79f614ce6d
#[test]
fn test_tagged_template_symbol_interpolation_does_not_report_ts2731() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            for rule in ["anyUnknownInErrorContext", "effectInFailure"] {
                t.run(rule, |t| {
                    t.parallel();

                    let diagnostics = collect_diagnostic_strings_from_content(
                        t,
                        &build_tagged_template_symbol_case(rule),
                    );
                    if has_diagnostic_code(&diagnostics, "TS2731:") {
                        t.fatal(format!(
                            "did not expect TS2731 with {rule} enabled, got {diagnostics:?}"
                        ));
                    }
                });
            }
        });
    });
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:58 buildTaggedTemplateSymbolCase 8af525704ae60c98
fn build_tagged_template_symbol_case(rule: &str) -> String {
    r#"// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          ""#
    .to_string()
        + rule
        + r#"": "error"
        }
      }
    ]
  }
}

// @filename: test.ts
const self: unique symbol = Symbol("self")
function sql(strings: TemplateStringsArray, ...args: Array<unknown>): string {
  return strings.join("?") + args.length
}
export const q = sql"#
        + "`(${self}) < 1`"
        + "\n"
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:82 TestIssue362RemedaChunkDoesNotReportExcessiveTypeDepth 98e72f2a550dc4c1
#[test]
fn test_issue362_remeda_chunk_does_not_report_excessive_type_depth() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            let with_plugin =
                collect_diagnostic_strings_from_content(t, &build_issue362_remeda_chunk_case(true));
            let without_plugin = collect_diagnostic_strings_from_content(
                t,
                &build_issue362_remeda_chunk_case(false),
            );

            for prefix in ["TS2321:", "TS2589:"] {
                if has_diagnostic_code(&without_plugin, prefix) {
                    t.fatal(format!(
                        "did not expect {prefix} without plugin, got {without_plugin:?}"
                    ));
                }
                if has_diagnostic_code(&with_plugin, prefix) {
                    t.fatal(format!(
                        "did not expect plugin to introduce {prefix}, got {with_plugin:?}"
                    ));
                }
            }
        });
    });
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:98 buildIssue362RemedaChunkCase 3ea76904785d1d06
fn build_issue362_remeda_chunk_case(plugin_enabled: bool) -> String {
    let mut plugin_config = "{}";
    if plugin_enabled {
        plugin_config = r#"{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}"#;
    }

    "// @filename: tsconfig.json\n".to_string()
        + plugin_config
        + r"

// @filename: test.ts
type IsAny<T> = 0 extends 1 & NoInfer<T> ? true : false
type IsNever<T> = [T] extends [never] ? true : false
type If<Type extends boolean, Then, Else> = IsNever<Type> extends true ? Else : Type extends true ? Then : Else
type IsNegative<T extends number> = T extends 0 ? false : "
        + "`${T}`"
        + " extends "
        + "`-${string}`"
        + r#" ? true : false
type TupleOf<Length extends number, Fill = unknown> = If<IsAny<Length>, Fill[], If<IsNever<Length>, [], _TupleOf<If<IsNegative<Length>, 0, Length>, Fill, []>>>
type _TupleOf<Length extends number, Fill, Accumulator extends readonly unknown[]> = number extends Length ? Fill[] : Length extends Accumulator["length"] ? Accumulator : _TupleOf<Length, Fill, [...Accumulator, Fill]>
type IntRange<From extends number, To extends number, List extends unknown[] = TupleOf<From, never>, End extends unknown[] = TupleOf<To>> = List["length"] extends To ? Exclude<List[number], never> : IntRange<From, To, [...List, List["length"]], End>
type IntRangeInclusive<From extends number, To extends number> = IntRange<From, To> | To
type NTuple<T, N extends number, Result extends unknown[] = []> = Result["length"] extends N ? Result : NTuple<T, N, [...Result, T]>
type Chunk<T extends readonly unknown[], N extends number> = T extends readonly [] ? [] : NTuple<T[number], N>[] | { [K in IntRangeInclusive<1, N>]: NTuple<T[number], K> }[IntRangeInclusive<1, N>]

declare function chunk<T extends readonly unknown[], N extends number>(array: T, size: N): Chunk<T, N>
declare function chunk<N extends number>(size: N): <T extends readonly unknown[]>(array: T) => Chunk<T, N>
declare const items: Array<string>
const result = chunk(items, 2)
"#
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:134 buildEffectInFailureTS2589Case 78a303b7abce08bb
pub(crate) fn build_effect_in_failure_ts2589_case(mode: &str) -> String {
    let mut plugin_config = "{}";
    match mode {
        "enabled" => {
            plugin_config = r#"{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}"#;
        }
        "rule-off" => {
            plugin_config = r#"{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "effectInFailure": "off"
        }
      }
    ]
  }
}"#;
        }
        _ => {}
    }

    let mut deep = "string".to_string();
    for _ in 0..25 {
        deep = "Wrap<".to_string() + &deep + ">";
    }

    let mut sb = String::new();
    sb.push_str("// @filename: tsconfig.json\n");
    sb.push_str(plugin_config);
    sb.push_str("\n\n// @filename: test.ts\n");
    sb.push_str("import { Data, Effect } from \"effect\"\n\n");
    sb.push_str("type TE<T> = Data.TaggedEnum<{\n");
    sb.push_str("  A: { a: T }\n");
    sb.push_str("  B: { b?: T }\n");
    sb.push_str("}>\n\n");
    sb.push_str("interface TEDefinition extends Data.TaggedEnum.WithGenerics<1> {\n");
    sb.push_str("  readonly taggedEnum: TE<this[\"A\"]>\n");
    sb.push_str("}\n\n");
    sb.push_str("type Wrap<T> = Data.TaggedEnum.Kind<TEDefinition, T>\n\n");
    sb.push_str("export const program = Effect.try({\n");
    sb.push_str("  try: () => 1,\n");
    let _ = writeln!(sb, "  catch: () => Effect.fail<{deep}>(undefined as any)");
    sb.push_str("})\n");
    sb
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:187 hasDiagnosticCode 8f5e61018428e3f0
pub(crate) fn has_diagnostic_code(diags: &[String], prefix: &str) -> bool {
    for diag in diags {
        if diag.contains(prefix) {
            return true;
        }
    }
    false
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:196 collectDiagnosticStringsFromContent 4cf57b2755ecc502
//
// Rust: as in `run_effect_test`, the Effect hooks are registered first (Go's blank import of `etscheckerhooks`), and
// the program and what the case allocates are freed at the end (Go: the GC).
pub(crate) fn collect_diagnostic_strings_from_content(t: &T, content: &str) -> Vec<String> {
    init_hooks();
    acquire_program();
    let _release = ReleaseProgramOnDrop;

    let units = parse_test_units(content, "test.ts");
    let current_directory = "/.src";
    let mut testfs = ets_bundledeffect::TestFS::default();
    if let Err(err) = ets_bundledeffect::mount_effect(EffectVersion::V4, &mut testfs) {
        t.fatal(err.to_string());
    }

    // Rust-only: what the case allocates outside the program goes to this region (see `run_effect_test`).
    let region = Region::new_scratch(1 << 20);
    let region_scope = region.enter();

    let mut program_file_names: Vec<String> = Vec::new();
    let mut ts_config_file: Option<P<TsConfigSourceFile>> = None;
    let mut ts_config_unit: Option<&TestUnit> = None;
    for unit in &units {
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        testfs.insert(unit_name.clone(), MapFile::from(unit.content.as_str()));
        if is_harness_config_file(&unit.name) {
            ts_config_file =
                Some(parse_ts_config_source_file(&unit_name, &unit.content, current_directory));
            ts_config_unit = Some(unit);
            continue;
        }
        program_file_names.push(unit_name);
    }

    let fs = vfstest::from_map(testfs, true);
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(fs));

    let mut compiler_options = P::new(CompilerOptions {
        new_line: NewLineKind::LF,
        skip_default_lib_check: Tristate::True,
        skip_lib_check: Tristate::True,
        no_error_truncation: Tristate::True,
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        es_module_interop: Tristate::True,
        allow_synthetic_default_imports: Tristate::True,
        ..CompilerOptions::default()
    });

    let mut parsed_config: Option<ParsedCommandLine> = None;
    if let (Some(ts_config_file), Some(ts_config_unit)) = (ts_config_file, ts_config_unit) {
        let config_dir = tspath::get_normalized_absolute_path(
            &tspath::get_directory_path(&ts_config_unit.name),
            current_directory,
        );
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
        if let Some(options) = parsed.compiler_options() {
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
    // Rust-only: regions freed with the program (see `run_effect_test`).
    program_options.checker_regions = true;
    program_options.checker_diagnostic_regions = true;
    let loader_regions = Arc::new(ProgramRegions::default());
    program_options.regions = Some(Arc::clone(&loader_regions));
    let program = ets_shim::compiler::new_program(program_options);

    let ctx = Context::background();
    let mut diagnostics = Vec::new();
    diagnostics.extend(program.get_program_diagnostics());
    diagnostics.extend(program.get_syntactic_diagnostics(&ctx, None));
    diagnostics.extend(program.get_semantic_diagnostics(&ctx, None));
    diagnostics.extend(program.get_global_diagnostics(&ctx));

    let mut results: Vec<String> = Vec::with_capacity(diagnostics.len());
    for diag in &diagnostics {
        results.push(format!("TS{}: {}", diag.code(), diag.localize()));
    }

    drop(diagnostics);
    drop(region_scope);
    // SAFETY: nothing read from the program outlives this call (the results are strings).
    unsafe { release_test_program(program, loader_regions, region) };
    results
}

// go test: internal/effecttest/effect_in_failure_ts2589_test.go:292 isHarnessConfigFile c881aaa284ec9a04
pub(crate) fn is_harness_config_file(file_name: &str) -> bool {
    file_name.ends_with("/tsconfig.json") || file_name == "tsconfig.json"
}
