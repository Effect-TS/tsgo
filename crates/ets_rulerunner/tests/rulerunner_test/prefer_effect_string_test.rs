// Port of internal/rulerunner/prefer_effect_string_test.go.

use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_go::testing::{on_checker_stack, run};
use ets_shim::P;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, ModuleKind, ModuleResolutionKind, ScriptTarget, Tristate};
use ets_shim::vfs::{FS, bundled, vfstest};
use rustc_hash::FxHashMap;

#[derive(Default)]
struct Case {
    name: &'static str,
    source: String,
    severity: Severity,
    disabled: bool,
    preset: &'static str,
    want: Vec<String>,
}

// go test: internal/rulerunner/prefer_effect_string_test.go:20 TestPreferEffectStringLifecycle
#[test]
fn test_prefer_effect_string_lifecycle() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();

            let cases = [
                Case {
                    name: "default off",
                    source: "\"hello\".trim();".to_string(),
                    ..Default::default()
                },
                Case {
                    name: "effect-native preset",
                    source: "\"hello\".trim();".to_string(),
                    preset: "effect-native",
                    want: vec!["377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "recommended preset",
                    source: "\"hello\".trim();".to_string(),
                    preset: "recommended",
                    ..Default::default()
                },
                Case {
                    name: "strict preset",
                    source: "\"hello\".trim();".to_string(),
                    preset: "strict",
                    ..Default::default()
                },
                Case {
                    name: "style preset",
                    source: "\"hello\".trim();".to_string(),
                    preset: "style",
                    ..Default::default()
                },
                Case {
                    name: "warning",
                    source: "\"hello\".trim();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "error",
                    source: "\"hello\".trim();".to_string(),
                    severity: Severity::Error,
                    want: vec!["377150:1:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "diagnostics disabled",
                    source: "\"hello\".trim();".to_string(),
                    severity: Severity::Warning,
                    disabled: true,
                    ..Default::default()
                },
                Case {
                    name: "at",
                    source: "\"hello\".at(0);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:10:Consider effect/String.at instead of native String.at. Returns Option, with None replacing undefined. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "charAt",
                    source: "\"hello\".charAt(0);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.charAt instead of native String.charAt. Returns Option, with None replacing the empty string for an out-of-range index. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "charCodeAt",
                    source: "\"hello\".charCodeAt(0);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:18:Consider effect/String.charCodeAt instead of native String.charCodeAt. Returns Option<number>, with None replacing NaN. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "codePointAt",
                    source: "\"hello\".codePointAt(0);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:19:Consider effect/String.codePointAt instead of native String.codePointAt. Returns Option<number>, with None replacing undefined. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "concat",
                    source: "\"hello\".concat(\"!\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.concat instead of native String.concat. Accepts exactly one string argument; native concat accepts any number of arguments and converts non-strings. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "endsWith",
                    source: "\"hello\".endsWith(\"o\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:16:Consider effect/String.endsWith instead of native String.endsWith. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "includes",
                    source: "\"hello\".includes(\"e\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:16:Consider effect/String.includes instead of native String.includes. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "indexOf",
                    source: "\"hello\".indexOf(\"l\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:15:Consider effect/String.indexOf instead of native String.indexOf. Returns Option<number>, with None replacing -1; there is no position parameter. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "lastIndexOf",
                    source: "\"hello\".lastIndexOf(\"l\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:19:Consider effect/String.lastIndexOf instead of native String.lastIndexOf. Returns Option<number>, with None replacing -1; there is no position parameter. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "localeCompare",
                    source: "\"hello\".localeCompare(\"world\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:21:Consider effect/String.localeCompare instead of native String.localeCompare. Returns an Ordering of -1, 0, or 1; Effect v4 types locales as Array<string>, so a single locale string needs an array. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "match",
                    source: "\"hello\".match(/l/);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:13:Consider effect/String.match instead of native String.match. Returns Option, with None replacing null. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "matchAll",
                    source: "\"hello\".matchAll(/l/g);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:16:Consider effect/String.matchAll instead of native String.matchAll. Requires a RegExp; string patterns are not accepted. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "normalize",
                    source: "\"hello\".normalize();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:17:Consider effect/String.normalize instead of native String.normalize. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "padEnd",
                    source: "\"hello\".padEnd(8);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.padEnd instead of native String.padEnd. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "padStart",
                    source: "\"hello\".padStart(8);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:16:Consider effect/String.padStart instead of native String.padStart. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "repeat",
                    source: "\"hello\".repeat(2);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.repeat instead of native String.repeat. In Effect v4, a negative count returns an empty string instead of throwing RangeError. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "replace",
                    source: "\"hello\".replace(\"l\", \"r\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:15:Consider effect/String.replace instead of native String.replace. Requires a string replacement; replacer callbacks are not accepted. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "replaceAll",
                    source: "\"hello\".replaceAll(\"l\", \"r\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:18:Consider effect/String.replaceAll instead of native String.replaceAll. Requires a string replacement; replacer callbacks are not accepted. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "search",
                    source: "\"hello\".search(/l/);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.search instead of native String.search. Returns Option<number>, with None replacing -1. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "slice",
                    source: "\"hello\".slice(1);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:13:Consider effect/String.slice instead of native String.slice. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "split",
                    source: "\"hello\".split(\"\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:13:Consider effect/String.split instead of native String.split. Returns a NonEmptyArray, so splitting an empty string by \"\" returns [\"\"] instead of []; there is no limit parameter. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "startsWith",
                    source: "\"hello\".startsWith(\"h\");".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:18:Consider effect/String.startsWith instead of native String.startsWith. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "substring",
                    source: "\"hello\".substring(1);".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:17:Consider effect/String.substring instead of native String.substring. Same runtime behavior. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "toLocaleLowerCase",
                    source: "\"hello\".toLocaleLowerCase();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:25:Consider effect/String.toLocaleLowerCase instead of native String.toLocaleLowerCase. Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "toLocaleUpperCase",
                    source: "\"hello\".toLocaleUpperCase();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:25:Consider effect/String.toLocaleUpperCase instead of native String.toLocaleUpperCase. Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "toLowerCase",
                    source: "\"hello\".toLowerCase();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:19:Consider effect/String.toLowerCase instead of native String.toLowerCase. Same runtime behavior; literal inputs keep a Lowercase<T> type. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "toUpperCase",
                    source: "\"hello\".toUpperCase();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:19:Consider effect/String.toUpperCase instead of native String.toUpperCase. Same runtime behavior; literal inputs keep an Uppercase<T> type. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "trim",
                    source: "\"hello\".trim();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "trimStart",
                    source: "\"hello\".trimStart();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:17:Consider effect/String.trimStart instead of native String.trimStart. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "trimEnd",
                    source: "\"hello\".trimEnd();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:15:Consider effect/String.trimEnd instead of native String.trimEnd. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "trimLeft",
                    source: "\"hello\".trimLeft();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:16:Consider effect/String.trimStart instead of native String.trimLeft. Deprecated alias of trimStart; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "trimRight",
                    source: "\"hello\".trimRight();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:17:Consider effect/String.trimEnd instead of native String.trimRight. Deprecated alias of trimEnd; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "bracket invocation",
                    source: "\"hello\"[\"trim\"]();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "parenthesized invocation",
                    source: "(\"hello\".trim)();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:9:13:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "optional call",
                    source: "\"hello\".trim?.();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "optional receiver",
                    source: "declare const s: string | undefined; s?.trim();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:40:44:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "optional bracket",
                    source: "declare const s: string | undefined; s?.[\"trim\"]();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:41:47:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "template name",
                    source: "\"hello\"[`trim`]();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:14:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "escaped name",
                    source: "\"hello\".tr\\u0069m();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:8:17:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "uncalled reference",
                    source: "const f = \"hello\".trim; f();".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "callback reference",
                    source: "[\"hello\"].map(\"hello\".trim);".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "borrowed call",
                    source: "String.prototype.trim.call(\"hello\");".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "excluded members",
                    source: "\"hello\".substr(1); \"hello\".toString(); \"hello\".anchor(\"id\"); \"hello\".isWellFormed();".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "other excluded members",
                    source: "\"hello\".valueOf(); \"hello\".toWellFormed(); \"hello\"[Symbol.iterator](); \"hello\".length;".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "constructor members",
                    source: "String.fromCharCode(65); String.fromCodePoint(65); String.raw`hello`;".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "mixed roots",
                    source: "declare const mixed: string | Array<string>; mixed.includes(\"h\"); mixed.slice(1);".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "any receiver",
                    source: "declare const s: any; s.trim();".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "augmentation",
                    source: "export {}; declare global { interface String { trim(): string; } } \"hello\".trim();".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "super receiver",
                    source: "class S extends String { test() { return super.trim(); } }".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "writes and delete",
                    source: "declare let s: Partial<Pick<String, \"trim\">>; s.trim = () => \"\"; delete (s.trim); const { trim } = s;".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "directive enables default off",
                    source: "// @effect-diagnostics preferEffectString:warning\n\"hello\".trim();".to_string(),
                    want: vec!["377150:0:58:62:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "directive disables warning",
                    source: "// @effect-diagnostics preferEffectString:off\n\"hello\".trim();".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "next line disables one invocation",
                    source: "// @effect-diagnostics-next-line preferEffectString:off\n\"hello\".trim();\n\"hello\".trim();".to_string(),
                    severity: Severity::Warning,
                    want: vec!["377150:0:80:84:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
                Case {
                    name: "next line enables default off",
                    source: "// @effect-diagnostics-next-line preferEffectString:error\n\"hello\".trim();\n\"hello\".trim();".to_string(),
                    want: vec!["377150:1:66:70:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)".to_string()],
                    ..Default::default()
                },
            ];
            for tc in &cases {
                t.run(tc.name, |t| {
                    t.parallel();
                    let mut severity = FxHashMap::default();
                    for r in ets_rules::ALL.iter() {
                        if r.name != "preferEffectString" {
                            severity.insert(r.name.to_string(), Severity::Off);
                        }
                    }
                    if tc.severity != Severity::Off {
                        severity.insert("preferEffectString".to_string(), tc.severity);
                    }
                    if !tc.preset.is_empty() {
                        for preset in ets_rules::metadata_presets() {
                            if preset.name == tc.preset {
                                severity.insert(
                                    "preferEffectString".to_string(),
                                    preset
                                        .diagnostic_severity
                                        .get("preferEffectString")
                                        .copied()
                                        .unwrap_or_default(),
                                );
                            }
                        }
                    }
                    ets_etscheckerhooks::init();
                    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(
                        [("/.src/test.ts", tc.source.as_str())],
                        true,
                    )));
                    let options = CompilerOptions {
                        effect: Some(Arc::new(EffectPluginOptions {
                            diagnostics: !tc.disabled,
                            diagnostic_severity: Some(severity),
                            ..Default::default()
                        }))
                        .into(),
                        target: ScriptTarget::ES_NEXT,
                        module: ModuleKind::NODE_NEXT,
                        module_resolution: ModuleResolutionKind::NODE_NEXT,
                        strict: Tristate::True,
                        skip_default_lib_check: Tristate::True,
                        skip_lib_check: Tristate::True,
                        ..Default::default()
                    };
                    let config = ets_shim::tsoptions::new_parsed_command_line(
                        P::new(options),
                        vec!["/.src/test.ts".to_string()],
                        Vec::new(),
                        ComparePathsOptions {
                            use_case_sensitive_file_names: true,
                            current_directory: "/.src".to_string(),
                        },
                    );
                    let mut program_options = ProgramOptions::new(
                        P::new(config),
                        ets_shim::compiler::new_compiler_host(
                            "/.src",
                            fs,
                            &bundled::lib_path(),
                            None,
                            None,
                            None,
                        ),
                    );
                    program_options.single_threaded = Tristate::True;
                    let program = crate::new_test_program(t, program_options);
                    let mut got = Vec::new();
                    for diagnostic in program.get_semantic_diagnostics(&Context::background(), None)
                    {
                        got.push(format!(
                            "{}:{}:{}:{}:{}",
                            diagnostic.code(),
                            diagnostic.category() as i32,
                            diagnostic.pos().unwrap_or_default(),
                            diagnostic.end().unwrap_or_default(),
                            diagnostic.localize()
                        ));
                    }
                    if got != tc.want {
                        t.fatal(format!("diagnostics = {got:?}, want {:?}", tc.want));
                    }
                });
            }
        });
    });
}
