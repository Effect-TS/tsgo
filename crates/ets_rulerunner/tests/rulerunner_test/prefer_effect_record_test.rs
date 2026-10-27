// Port of internal/rulerunner/prefer_effect_record_test.go.

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

// go test: internal/rulerunner/prefer_effect_record_test.go:20 TestPreferEffectRecordLifecycle deb2ec2f4726eafc
#[test]
fn test_prefer_effect_record_lifecycle() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            const KEYS_MESSAGE: &str = "Consider effect/Record.keys instead of native Object.keys. Requires a record rather than arrays or primitives; types keys as the inferred key union, which does not guarantee exact runtime keys. effect(preferEffectRecord)";
            const VALUES_MESSAGE: &str = "Consider effect/Record.values instead of native Object.values. Requires a record rather than arrays or primitives; snapshots keys before reads, so getter or proxy mutations can change the result. effect(preferEffectRecord)";
            const ENTRIES_MESSAGE: &str = "Consider effect/Record.toEntries instead of native Object.entries. Requires a record rather than arrays or primitives; types entries with inferred keys and snapshots keys before reads, so getter or proxy mutations can change the result. effect(preferEffectRecord)";
            const FROM_ENTRIES_MESSAGE: &str = "Consider effect/Record.fromEntries instead of native Object.fromEntries. Requires typed string or symbol key tuples; native numeric keys and loose entry arrays need adaptation. effect(preferEffectRecord)";
            const HAS_MESSAGE: &str = "Consider effect/Record.has instead of native Object.hasOwn. Requires a record rather than arrays or primitives and a string or symbol key of that record's key type; arbitrary strings may need narrowing. effect(preferEffectRecord)";
            const PROTOTYPE_MESSAGE: &str = "Consider effect/Record.has instead of native Object.prototype.hasOwnProperty. Pass the receiver explicitly; requires a record rather than arrays or primitives and a string or symbol key of that record's key type. Avoids instance binding and overrides. effect(preferEffectRecord)";
            let cases = [
                Case {
                    name: "default off",
                    source: "Object.keys({ a: 1 });".to_string(),
                    ..Default::default()
                },
                Case {
                    name: "effect native preset",
                    source: "Object.keys({ a: 1 });".to_string(),
                    preset: "effect-native",
                    want: vec![["377143:0:7:11:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "recommended preset",
                    source: "Object.keys({ a: 1 });".to_string(),
                    preset: "recommended",
                    ..Default::default()
                },
                Case {
                    name: "strict preset",
                    source: "Object.keys({ a: 1 });".to_string(),
                    preset: "strict",
                    ..Default::default()
                },
                Case {
                    name: "warning",
                    source: "Object.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:11:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "error",
                    source: "Object.keys({ a: 1 });".to_string(),
                    severity: Severity::Error,
                    want: vec![["377143:1:7:11:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "diagnostics disabled",
                    source: "Object.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    disabled: true,
                    ..Default::default()
                },
                Case {
                    name: "dot reference",
                    source: "const alias = Object.keys; alias({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:21:25:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "bracket invocation",
                    source: "Object[\"entries\"]({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:16:", ENTRIES_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "bracket reference",
                    source: "const alias = Object[\"keys\"]; alias({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:21:27:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "values",
                    source: "Object.values({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:13:", VALUES_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "fromEntries",
                    source: "Object.fromEntries([[1, \"a\"]]);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:18:", FROM_ENTRIES_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "hasOwn",
                    source: "Object.hasOwn({ a: 1 }, \"a\");".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:13:", HAS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "borrowed prototype",
                    source: "Object.prototype.hasOwnProperty.call({ a: 1 }, \"a\");".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:17:31:", PROTOTYPE_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "inherited method",
                    source: "const data = { a: 1 }; data.hasOwnProperty(\"a\");".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:28:42:", PROTOTYPE_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "optional bracket",
                    source: "declare const native: ObjectConstructor | undefined; native?.[\"keys\"]({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:62:68:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "generic bracket",
                    source: "function constrained<T extends ObjectConstructor>(native: T) { return native[\"keys\"]; }".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:77:83:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "array and primitive arguments",
                    source: "Object.keys([1]); Object.values(\"a\"); Object.hasOwn([], \"a\");".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:11:", KEYS_MESSAGE].concat(), ["377143:0:25:31:", VALUES_MESSAGE].concat(), ["377143:0:45:51:", HAS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "escaped and template names",
                    source: "Object.k\\u0065ys({ a: 1 }); Object[\"k\\u0065ys\"]({ a: 1 }); Object[`keys`]({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:7:16:", KEYS_MESSAGE].concat(), ["377143:0:35:46:", KEYS_MESSAGE].concat(), ["377143:0:66:72:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "parenthesized delete target",
                    source: "declare const native: Partial<Pick<ObjectConstructor, \"keys\">>; delete (native.keys); native.keys;".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:93:97:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "type write and destructuring positions",
                    source: "type Keys = typeof Object.keys; Object.keys = () => []; ({ keys: Object.keys } = Object); const { keys } = Object; Object.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:122:126:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "super access",
                    source: "class Base {} class Child extends Base { test() { return super.hasOwnProperty(\"a\"); } } Object.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:95:99:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "shadow and mixed roots",
                    source: "function shadowed(Object: { keys(value: object): string[] }) { return Object.keys({}); } declare const mixed: ObjectConstructor | { keys(value: object): string[] }; mixed.keys({}); Object.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:188:192:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "directive enables default off",
                    source: "// @effect-diagnostics preferEffectRecord:warning\nObject.keys({ a: 1 });".to_string(),
                    want: vec![["377143:0:57:61:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "directive disables warning",
                    source: "// @effect-diagnostics preferEffectRecord:off\nObject.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "next line disables one reference",
                    source: "// @effect-diagnostics-next-line preferEffectRecord:off\nObject.keys({ a: 1 });\nObject.keys({ a: 1 });".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377143:0:86:90:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "next line enables default off",
                    source: "// @effect-diagnostics-next-line preferEffectRecord:error\nObject.keys({ a: 1 });\nObject.keys({ a: 1 });".to_string(),
                    want: vec![["377143:1:65:69:", KEYS_MESSAGE].concat()],
                    ..Default::default()
                },
            ];
            for tc in &cases {
                t.run(tc.name, |t| {
                    t.parallel();
                    let mut severity = FxHashMap::default();
                    for r in ets_rules::ALL.iter() {
                        if r.name != "preferEffectRecord" {
                            severity.insert(r.name.to_string(), Severity::Off);
                        }
                    }
                    if tc.severity != Severity::Off {
                        severity.insert("preferEffectRecord".to_string(), tc.severity);
                    }
                    if !tc.preset.is_empty() {
                        for preset in ets_rules::metadata_presets() {
                            if preset.name == tc.preset {
                                severity.insert(
                                    "preferEffectRecord".to_string(),
                                    preset
                                        .diagnostic_severity
                                        .get("preferEffectRecord")
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
