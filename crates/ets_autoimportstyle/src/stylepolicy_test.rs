use std::sync::Arc;

use ets_etscore::{ResolvedEffectPluginOptions, TopLevelNamedReexportsMode};
use ets_go::sprintf;
use ets_shim::ast::SymbolFlags;
use ets_shim::ls::autoimport::{Export, ExportID, ExportSyntax, Fix, ModuleID};
use ets_shim::lsproto;
use ets_shim::modulespecifiers::ResultKind;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::*;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn string_map(entries: &[(&str, &str)]) -> FxHashMap<String, String> {
    entries.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
}

/// Rust-only: the shim's `autoimport.NewAmbientModuleID`.
fn new_ambient_module_id(specifier: &str) -> ModuleID {
    if specifier.is_empty() {
        return ModuleID::default();
    }
    ModuleID::Ambient(specifier.to_string())
}

fn dbg(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

// go test: internal/autoimportstyle/stylepolicy_test.go:15 TestStylePolicyIsEmpty e66ecdfe0cb46176
#[test]
fn test_style_policy_is_empty() {
    ets_go::testing::run(|t| {
        t.parallel();
        let nil_policy: Option<&StylePolicy> = None;
        if !StylePolicy::is_empty(nil_policy) {
            t.error("nil policy should be empty");
        }

        let empty = new_style_policy(Some(&ResolvedEffectPluginOptions::default()));
        if !StylePolicy::is_empty(Some(&empty)) {
            t.error("empty preferences should produce empty policy");
        }

        let non_empty = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));
        if StylePolicy::is_empty(Some(&non_empty)) {
            t.error("non-empty preferences should not be empty");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:35 makeExport ff2d7fd984dda74d
fn make_export(package_name: &str, module_id: &str, target_module_id: &str) -> Export {
    let mut export = Export::default();
    export.export_id.module_id = new_ambient_module_id(module_id);
    export.export_id.export_name = "succeed".to_string();
    export.package_name = package_name.to_string();
    export.target =
        ExportID { module_id: new_ambient_module_id(target_module_id), ..Default::default() };
    export
}

// go test: internal/autoimportstyle/stylepolicy_test.go:46 makeAddNewFix e1a0a09857f61dc6
fn make_add_new_fix(import_kind: lsproto::ImportKind, module_specifier: &str, name: &str) -> Fix {
    let usage_position = lsproto::Position { line: 1, character: 1 };
    Fix {
        auto_import_fix: lsproto::AutoImportFix {
            kind: lsproto::AutoImportFixKind::ADD_NEW,
            import_kind,
            module_specifier: module_specifier.to_string(),
            name: name.to_string(),
            usage_position: Some(usage_position),
            ..Default::default()
        },
        ..Default::default()
    }
}

// go test: internal/autoimportstyle/stylepolicy_test.go:59 TestApplyNamespaceReexport 36cbc80197764799
#[test]
fn test_apply_namespace_reexport() {
    ets_go::testing::run(|t| {
        t.parallel();
        let mut sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));
        let unexpected: Arc<Mutex<Option<String>>> = Arc::default();
        let unexpected_in_callback = Arc::clone(&unexpected);
        sp.resolve_target = Some(Box::new(move |export: &Export| {
            if module_id_string(&export.target.module_id) != "effect/Effect" {
                // Rust: Go calls t.Fatalf from the callback; the test reports it after Apply returns.
                *unexpected_in_callback.lock() =
                    Some(module_id_string(&export.target.module_id).to_string());
            }
            ("effect/Effect".to_string(), ResultKind::NodeModules)
        }));

        let mut export = make_export("effect", "effect", "effect/Effect");
        export.export_id.export_name = "Effect".to_string();
        export.syntax = ExportSyntax::Modifier;
        export.flags = SymbolFlags::ValueModule;
        let fix = Arc::new(make_add_new_fix(lsproto::ImportKind::NAMED, "effect", "Effect"));

        let result = sp.apply(Some(&export), Some(fix));
        let unexpected_module_id = unexpected.lock().take();
        if let Some(module_id) = unexpected_module_id {
            t.fatal(sprintf!("unexpected target module: %q", module_id));
        }
        let Some(result) = result else {
            t.fatal("expected non-nil result");
        };
        if result.import_kind != lsproto::ImportKind::NAMESPACE {
            t.error(sprintf!("expected ImportKindNamespace, got %v", dbg(&result.import_kind)));
        }
        if result.module_specifier != "effect/Effect" {
            t.error(sprintf!(
                "expected module specifier 'effect/Effect', got %q",
                result.module_specifier
            ));
        }
        if result.name != "Effect" {
            t.error(sprintf!("expected namespace name 'Effect', got %q", result.name));
        }
        if !result.namespace_prefix.is_empty() {
            t.error(sprintf!("expected no usage qualifier, got %q", result.namespace_prefix));
        }
        if result.module_specifier_kind != ResultKind::NodeModules {
            t.error(sprintf!(
                "expected node_modules specifier kind, got %v",
                dbg(&result.module_specifier_kind)
            ));
        }
        if result.is_re_export {
            t.error("expected rewritten direct import not to be marked as a reexport");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:101 newBarrelPolicy 934b69e60f6b14c2
fn new_barrel_policy(options: &ResolvedEffectPluginOptions, disk: &[(&str, &str)]) -> StylePolicy {
    let mut sp = new_style_policy(Some(options));
    let disk = string_map(disk);
    sp.barrels = Some(Mutex::new(ets_importstyle::new_barrel_reader(
        |_| None,
        move |file_name| disk.get(file_name).cloned(),
    )));
    sp
}

// go test: internal/autoimportstyle/stylepolicy_test.go:113 makeBarrelFix 64087d0c4013252e
fn make_barrel_fix(module_specifier: &str, module_file_name: &str, name: &str) -> Fix {
    let mut fix = make_add_new_fix(lsproto::ImportKind::NAMED, module_specifier, name);
    fix.module_file_name = module_file_name.to_string();
    fix
}

// go test: internal/autoimportstyle/stylepolicy_test.go:119 TestApplyNamespaceRewrite 48440aec1e9f17b8
#[test]
#[expect(
    clippy::suspicious_operation_groupings,
    reason = "compares the result's name with the export name, as Go"
)]
fn test_apply_namespace_rewrite() {
    ets_go::testing::run(|t| {
        t.parallel();
        struct Case {
            name: &'static str,
            options: ResolvedEffectPluginOptions,
            module_specifier: &'static str,
            export_name: &'static str,
            fix_kind: lsproto::AutoImportFixKind,
            want_prefix: &'static str,
        }
        let tests = [
            Case {
                name: "add new",
                options: ResolvedEffectPluginOptions {
                    namespace_import_packages: strings(&["effect"]),
                    ..Default::default()
                },
                module_specifier: "effect/Effect",
                export_name: "succeed",
                fix_kind: lsproto::AutoImportFixKind::ADD_NEW,
                want_prefix: "Effect",
            },
            Case {
                name: "add to existing",
                options: ResolvedEffectPluginOptions {
                    namespace_import_packages: strings(&["effect"]),
                    ..Default::default()
                },
                module_specifier: "effect/testing/TestClock",
                export_name: "testClockWith",
                fix_kind: lsproto::AutoImportFixKind::ADD_TO_EXISTING,
                want_prefix: "TestClock",
            },
            Case {
                name: "alias",
                options: ResolvedEffectPluginOptions {
                    namespace_import_packages: strings(&["effect"]),
                    import_aliases: string_map(&[("effect", "Fx")]),
                    ..Default::default()
                },
                module_specifier: "effect/Effect",
                export_name: "succeed",
                fix_kind: lsproto::AutoImportFixKind::ADD_NEW,
                want_prefix: "Fx",
            },
            Case {
                name: "case-insensitive package",
                options: ResolvedEffectPluginOptions {
                    namespace_import_packages: strings(&["Effect"]),
                    ..Default::default()
                },
                module_specifier: "effect/Effect",
                export_name: "succeed",
                fix_kind: lsproto::AutoImportFixKind::ADD_NEW,
                want_prefix: "Effect",
            },
        ];
        for tt in &tests {
            t.run(tt.name, |t| {
                t.parallel();
                let mut fix = make_add_new_fix(
                    lsproto::ImportKind::NAMED,
                    tt.module_specifier,
                    tt.export_name,
                );
                fix.auto_import_fix.kind = tt.fix_kind;
                let result = new_style_policy(Some(&tt.options)).apply(
                    Some(&make_export("effect", tt.module_specifier, "")),
                    Some(Arc::new(fix)),
                );
                let Some(result) = result else {
                    t.fatal("expected non-nil result");
                };
                if result.kind != lsproto::AutoImportFixKind::ADD_NEW
                    || result.import_kind != lsproto::ImportKind::NAMESPACE
                {
                    t.error(sprintf!(
                        "expected AddNew namespace import, got kind %v import kind %v",
                        dbg(&result.kind),
                        dbg(&result.import_kind)
                    ));
                }
                if result.name != tt.export_name
                    || result.module_specifier != tt.module_specifier
                    || result.namespace_prefix != tt.want_prefix
                {
                    t.error(sprintf!(
                        "got name %q specifier %q prefix %q, want %q %q %q",
                        result.name,
                        result.module_specifier,
                        result.namespace_prefix,
                        tt.export_name,
                        tt.module_specifier,
                        tt.want_prefix
                    ));
                }
            });
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:184 TestApplyBarrelRewrite 231848707b1aa425
#[test]
fn test_apply_barrel_rewrite() {
    ets_go::testing::run(|t| {
        t.parallel();
        let platform_disk: &[(&str, &str)] = &[(
            "/node_modules/@effect/platform/dist/dts/index.d.ts",
            r#"export * as HttpClient from "./HttpClient.js""#,
        )];
        let acme_disk: &[(&str, &str)] = &[
            ("/node_modules/acme/dist/index.d.ts", r#"export * as Http from "./HttpClient.ts""#),
            (
                "/node_modules/acme/dist/forms/index.d.ts",
                r#"export * as TextInput from "./TextInput.ts""#,
            ),
        ];
        struct Case<'a> {
            name: &'static str,
            options: ResolvedEffectPluginOptions,
            disk: &'a [(&'a str, &'a str)],
            package_name: &'static str,
            module_specifier: &'static str,
            module_file_name: &'static str,
            want_barrel: &'static str,
            want_prefix: &'static str,
        }
        let tests = [
            Case {
                name: "package root barrel",
                options: ResolvedEffectPluginOptions {
                    barrel_import_packages: strings(&["@effect/platform"]),
                    ..Default::default()
                },
                disk: platform_disk,
                package_name: "@effect/platform",
                module_specifier: "@effect/platform/HttpClient",
                module_file_name: "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
                want_barrel: "@effect/platform",
                want_prefix: "HttpClient",
            },
            Case {
                name: "alias",
                options: ResolvedEffectPluginOptions {
                    barrel_import_packages: strings(&["@effect/platform"]),
                    import_aliases: string_map(&[("@effect/platform", "Platform")]),
                    ..Default::default()
                },
                disk: platform_disk,
                package_name: "@effect/platform",
                module_specifier: "@effect/platform/HttpClient",
                module_file_name: "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
                want_barrel: "@effect/platform",
                want_prefix: "Platform",
            },
            Case {
                name: "export name differs from module",
                options: ResolvedEffectPluginOptions {
                    barrel_import_packages: strings(&["acme"]),
                    ..Default::default()
                },
                disk: acme_disk,
                package_name: "acme",
                module_specifier: "acme/HttpClient",
                module_file_name: "/node_modules/acme/dist/HttpClient.d.ts",
                want_barrel: "acme",
                want_prefix: "Http",
            },
            Case {
                name: "nearest forwarding barrel",
                options: ResolvedEffectPluginOptions {
                    barrel_import_packages: strings(&["acme"]),
                    ..Default::default()
                },
                disk: acme_disk,
                package_name: "acme",
                module_specifier: "acme/forms/TextInput",
                module_file_name: "/node_modules/acme/dist/forms/TextInput.d.ts",
                want_barrel: "acme/forms",
                want_prefix: "TextInput",
            },
        ];
        for tt in &tests {
            t.run(tt.name, |t| {
                t.parallel();
                let sp = new_barrel_policy(&tt.options, tt.disk);
                let result = sp.apply(
                    Some(&make_export(tt.package_name, tt.module_specifier, "")),
                    Some(Arc::new(make_barrel_fix(
                        tt.module_specifier,
                        tt.module_file_name,
                        "request",
                    ))),
                );
                let Some(result) = result else {
                    t.fatal("expected non-nil result");
                };
                if result.import_kind != lsproto::ImportKind::NAMED || result.name != "request" {
                    t.error(sprintf!(
                        "expected named import of 'request', got import kind %v name %q",
                        dbg(&result.import_kind),
                        result.name
                    ));
                }
                if result.module_specifier != tt.want_barrel
                    || result.namespace_prefix != tt.want_prefix
                {
                    t.error(sprintf!(
                        "got specifier %q prefix %q, want %q %q",
                        result.module_specifier,
                        result.namespace_prefix,
                        tt.want_barrel,
                        tt.want_prefix
                    ));
                }
            });
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:263 TestApplyPassthrough 2b5fa03d4b3febff
#[test]
fn test_apply_passthrough() {
    ets_go::testing::run(|t| {
        t.parallel();
        let namespace_policy = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            top_level_named_reexports: TopLevelNamedReexportsMode::Ignore,
            ..Default::default()
        }));
        let platform_policy = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["@effect/platform"]),
                ..Default::default()
            },
            &[(
                "/node_modules/@effect/platform/dist/dts/index.d.ts",
                r#"export * as HttpClient from "./HttpClient.js""#,
            )],
        );
        let acme_policy = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["acme"]),
                ..Default::default()
            },
            &[("/node_modules/acme/dist/index.d.ts", r#"export * as Button from "./Button.ts""#)],
        );
        let effect_barrel_policy = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["effect"]),
                ..Default::default()
            },
            &[(
                "/node_modules/effect/dist/index.d.ts",
                r#"export * as Function from "./Function.js""#,
            )],
        );

        let without_usage = |mut fix: Fix| -> Fix {
            fix.auto_import_fix.usage_position = None;
            fix
        };
        let with_kind = |mut fix: Fix, kind: lsproto::AutoImportFixKind| -> Fix {
            fix.auto_import_fix.kind = kind;
            fix
        };
        let named_reexport = |module_id: &str, target_module_id: &str| -> Export {
            let mut export = make_export("effect", module_id, target_module_id);
            export.syntax = ExportSyntax::Named;
            export
        };

        let tests: Vec<(&str, &StylePolicy, Export, Fix)> = vec![
            (
                "package without style",
                &namespace_policy,
                make_export("lodash", "lodash", ""),
                make_add_new_fix(lsproto::ImportKind::NAMED, "lodash", "map"),
            ),
            (
                "use namespace fix",
                &namespace_policy,
                make_export("effect", "effect/Effect", ""),
                with_kind(
                    make_add_new_fix(lsproto::ImportKind::NAMED, "effect/Effect", "succeed"),
                    lsproto::AutoImportFixKind::USE_NAMESPACE,
                ),
            ),
            (
                "namespace without usage position",
                &namespace_policy,
                make_export("effect", "effect/Effect", ""),
                without_usage(make_add_new_fix(
                    lsproto::ImportKind::NAMED,
                    "effect/Effect",
                    "succeed",
                )),
            ),
            (
                "ignored top-level reexport",
                &namespace_policy,
                named_reexport("effect", "effect/Effect"),
                make_add_new_fix(lsproto::ImportKind::NAMED, "effect", "succeed"),
            ),
            (
                "barrel without usage position",
                &platform_policy,
                make_export("@effect/platform", "@effect/platform/HttpClient", ""),
                without_usage(make_barrel_fix(
                    "@effect/platform/HttpClient",
                    "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
                    "request",
                )),
            ),
            (
                "module no barrel forwards",
                &acme_policy,
                make_export("acme", "acme/Private", ""),
                make_barrel_fix("acme/Private", "/node_modules/acme/dist/Private.d.ts", "secret"),
            ),
            (
                "from barrel effect",
                &effect_barrel_policy,
                named_reexport("effect", "effect/Function"),
                make_barrel_fix("effect", "/node_modules/effect/dist/index.d.ts", "pipe"),
            ),
            (
                "from barrel effect/testing",
                &effect_barrel_policy,
                named_reexport("effect/testing", "effect/Function"),
                make_barrel_fix(
                    "effect/testing",
                    "/node_modules/effect/dist/testing/index.d.ts",
                    "pipe",
                ),
            ),
            (
                "from barrel effect/http",
                &effect_barrel_policy,
                named_reexport("effect/http", "effect/Function"),
                make_barrel_fix("effect/http", "/node_modules/effect/dist/http/index.d.ts", "pipe"),
            ),
        ];
        for (name, policy, export, fix) in tests {
            t.run(name, |t| {
                t.parallel();
                let fix = Arc::new(fix);
                let result = policy.apply(Some(&export), Some(Arc::clone(&fix)));
                if !result.as_ref().is_some_and(|result| Arc::ptr_eq(result, &fix)) {
                    t.error(sprintf!(
                        "expected fix to be returned unchanged, got %#v",
                        dbg(&result)
                    ));
                }
            });
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:319 TestApplySuppressed b960e379870d2b71
#[test]
fn test_apply_suppressed() {
    ets_go::testing::run(|t| {
        t.parallel();
        let mut follow_reexport = make_export("effect", "effect", "effect/Effect");
        follow_reexport.syntax = ExportSyntax::Named;
        struct Case {
            name: &'static str,
            options: ResolvedEffectPluginOptions,
            export: Export,
            fix: Fix,
        }
        let tests = vec![
            Case {
                name: "effect internal module",
                options: ResolvedEffectPluginOptions {
                    namespace_import_packages: strings(&["effect"]),
                    ..Default::default()
                },
                export: make_export("effect", "effect/internal/schema/parser", ""),
                fix: make_add_new_fix(
                    lsproto::ImportKind::NAMED,
                    "effect/internal/schema/parser",
                    "succeed",
                ),
            },
            Case {
                name: "followed top-level reexport",
                options: ResolvedEffectPluginOptions {
                    namespace_import_packages: strings(&["effect"]),
                    top_level_named_reexports: TopLevelNamedReexportsMode::Follow,
                    ..Default::default()
                },
                export: follow_reexport,
                fix: make_add_new_fix(lsproto::ImportKind::NAMED, "effect", "succeed"),
            },
        ];
        for tt in tests {
            t.run(tt.name, |t| {
                t.parallel();
                if let Some(result) = new_style_policy(Some(&tt.options))
                    .apply(Some(&tt.export), Some(Arc::new(tt.fix)))
                {
                    t.error(sprintf!("expected fix to be suppressed, got %#v", dbg(&result)));
                }
            });
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:355 TestApplyNilInputs b61459d6e6745119
#[test]
fn test_apply_nil_inputs() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let fix =
            Arc::new(make_add_new_fix(lsproto::ImportKind::NAMED, "effect/Effect", "succeed"));
        if sp.apply(None, Some(fix)).is_none() {
            t.error("expected fix returned when export is nil");
        }

        let export = make_export("effect", "effect/Effect", "");
        if sp.apply(Some(&export), None).is_some() {
            t.error("expected nil when fix is nil");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:371 TestNewFixTransformerNilForEmptyPrefs 11879c762289cce9
#[test]
fn test_new_fix_transformer_nil_for_empty_prefs() {
    ets_go::testing::run(|t| {
        t.parallel();
        let transformer =
            new_fix_transformer(Some(&ResolvedEffectPluginOptions::default()), None, None);
        if transformer.is_some() {
            t.error("expected nil transformer for empty preferences");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:379 TestNewFixTransformerAppliesPolicy b69a773c3e058a13
#[test]
fn test_new_fix_transformer_applies_policy() {
    ets_go::testing::run(|t| {
        t.parallel();
        let transformer = new_fix_transformer(
            Some(&ResolvedEffectPluginOptions {
                namespace_import_packages: strings(&["effect"]),
                ..Default::default()
            }),
            None,
            None,
        );
        let Some(transformer) = transformer else {
            t.fatal("expected non-nil transformer");
        };

        let export = make_export("effect", "effect/Effect", "");
        let fixes = vec![Arc::new(make_add_new_fix(
            lsproto::ImportKind::NAMED,
            "effect/Effect",
            "succeed",
        ))];

        let result = transformer(Some(&export), fixes);
        if result.len() != 1 {
            t.fatal(sprintf!("expected 1 fix, got %d", result.len()));
        }
        if result[0].import_kind != lsproto::ImportKind::NAMESPACE {
            t.error(sprintf!("expected ImportKindNamespace, got %v", dbg(&result[0].import_kind)));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:400 TestNewFixTransformerPrefersExistingNamespaceUse cac50b277e2fadc0
#[test]
fn test_new_fix_transformer_prefers_existing_namespace_use() {
    ets_go::testing::run(|t| {
        t.parallel();
        let transformer = new_fix_transformer(
            Some(&ResolvedEffectPluginOptions {
                namespace_import_packages: strings(&["effect"]),
                ..Default::default()
            }),
            None,
            None,
        );
        let Some(transformer) = transformer else {
            t.fatal("expected non-nil transformer");
        };

        let export = make_export("effect", "effect/testing/TestClock", "");
        let use_namespace = Arc::new(Fix {
            auto_import_fix: lsproto::AutoImportFix {
                kind: lsproto::AutoImportFixKind::USE_NAMESPACE,
                import_kind: lsproto::ImportKind::NAMESPACE,
                module_specifier: "effect/testing/TestClock".to_string(),
                name: "testClockWith".to_string(),
                namespace_prefix: "TestClock".to_string(),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut add_to_existing = make_add_new_fix(
            lsproto::ImportKind::NAMED,
            "effect/testing/TestClock",
            "testClockWith",
        );
        add_to_existing.auto_import_fix.kind = lsproto::AutoImportFixKind::ADD_TO_EXISTING;

        let result = transformer(Some(&export), vec![use_namespace, Arc::new(add_to_existing)]);
        if result.len() != 1 {
            t.fatal(sprintf!("expected 1 fix, got %d", result.len()));
        }
        if result[0].kind != lsproto::AutoImportFixKind::USE_NAMESPACE {
            t.fatal(sprintf!("expected UseNamespace fix, got %v", dbg(&result[0].kind)));
        }
    });
}
