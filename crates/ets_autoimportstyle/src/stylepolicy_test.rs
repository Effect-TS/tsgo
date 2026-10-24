use std::sync::Arc;

use ets_etscore::{ResolvedEffectPluginOptions, TopLevelNamedReexportsMode};
use ets_go::sprintf;
use ets_importstyle::Style;
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

/// Rust-only: the shim's `autoimport.NewAmbientModuleID` (tsrs's `ModuleID` is the string itself).
fn new_ambient_module_id(specifier: &str) -> ModuleID {
    ModuleID(specifier.to_string())
}

fn dbg(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

// go test: internal/autoimportstyle/stylepolicy_test.go:15 TestNewStylePolicy
#[test]
fn test_new_style_policy() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect", "Effect"]),
            barrel_import_packages: strings(&["@effect/platform"]),
            import_aliases: string_map(&[("effect", "Fx")]),
            ..Default::default()
        }));

        if sp.styles.package_style("effect") != Style::Namespace {
            t.error("expected 'effect' in namespace packages");
        }
        if sp.styles.package_style("EFFECT") != Style::Namespace {
            t.error("expected case-insensitive match for 'EFFECT'");
        }
        if sp.styles.package_style("@effect/platform") != Style::Barrel {
            t.error("expected '@effect/platform' in barrel packages");
        }
        let alias = sp.aliases.get("effect").cloned().unwrap_or_default();
        if alias != "Fx" {
            t.error(sprintf!("expected alias 'Fx' for 'effect', got %q", alias));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:37 TestStylePolicyIsEmpty
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

// go test: internal/autoimportstyle/stylepolicy_test.go:57 makeExport
fn make_export(package_name: &str, module_id: &str, target_module_id: &str) -> Export {
    let mut export = Export::default();
    export.export_id.module_id = new_ambient_module_id(module_id);
    export.export_id.export_name = "succeed".to_string();
    export.package_name = package_name.to_string();
    export.target =
        ExportID { module_id: new_ambient_module_id(target_module_id), ..Default::default() };
    export
}

// go test: internal/autoimportstyle/stylepolicy_test.go:68 makeAddNewFix
fn make_add_new_fix(import_kind: lsproto::ImportKind, module_specifier: &str, name: &str) -> Fix {
    let usage_position = lsproto::Position { line: 1, character: 1 };
    Fix {
        auto_import_fix: lsproto::AutoImportFix {
            kind: lsproto::AutoImportFixKind::AddNew,
            import_kind,
            module_specifier: module_specifier.to_string(),
            name: name.to_string(),
            usage_position: Some(usage_position),
            ..Default::default()
        },
        ..Default::default()
    }
}

// go test: internal/autoimportstyle/stylepolicy_test.go:81 TestApplyNamespaceRewrite
#[test]
fn test_apply_namespace_rewrite() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let export = make_export("effect", "effect/Effect", "");
        let fix =
            Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect/Effect", "succeed"));

        let Some(result) = sp.apply(Some(&export), Some(fix)) else {
            t.fatal("expected non-nil result");
        };
        if result.import_kind != lsproto::ImportKind::Namespace {
            t.error(sprintf!("expected ImportKindNamespace, got %v", dbg(&result.import_kind)));
        }
        if result.name != "succeed" {
            t.error(sprintf!(
                "expected name 'succeed' (original export name), got %q",
                result.name
            ));
        }
        if result.namespace_prefix != "Effect" {
            t.error(sprintf!(
                "expected namespace prefix 'Effect', got %q",
                result.namespace_prefix
            ));
        }
        if result.module_specifier != "effect/Effect" {
            t.error(sprintf!(
                "expected module specifier 'effect/Effect', got %q",
                result.module_specifier
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:109 TestApplySuppressesEffectInternalImport
#[test]
fn test_apply_suppresses_effect_internal_import() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let export = make_export("effect", "effect/internal/schema/parser", "");
        let fix = Arc::new(make_add_new_fix(
            lsproto::ImportKind::Named,
            "effect/internal/schema/parser",
            "succeed",
        ));

        if let Some(result) = sp.apply(Some(&export), Some(fix)) {
            t.error(sprintf!(
                "expected blocked Effect internal import to be suppressed, got %#v",
                dbg(&result)
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:123 TestApplyNamespaceRewriteFromAddToExisting
#[test]
fn test_apply_namespace_rewrite_from_add_to_existing() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let export = make_export("effect", "effect/testing/TestClock", "");
        let mut fix = make_add_new_fix(
            lsproto::ImportKind::Named,
            "effect/testing/TestClock",
            "testClockWith",
        );
        fix.auto_import_fix.kind = lsproto::AutoImportFixKind::AddToExisting;

        let Some(result) = sp.apply(Some(&export), Some(Arc::new(fix))) else {
            t.fatal("expected non-nil result");
        };
        if result.kind != lsproto::AutoImportFixKind::AddNew {
            t.error(sprintf!("expected rewrite to AddNew, got %v", dbg(&result.kind)));
        }
        if result.import_kind != lsproto::ImportKind::Namespace {
            t.error(sprintf!("expected ImportKindNamespace, got %v", dbg(&result.import_kind)));
        }
        if result.module_specifier != "effect/testing/TestClock" {
            t.error(sprintf!(
                "expected module specifier 'effect/testing/TestClock', got %q",
                result.module_specifier
            ));
        }
        if result.namespace_prefix != "TestClock" {
            t.error(sprintf!(
                "expected namespace prefix 'TestClock', got %q",
                result.namespace_prefix
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:152 TestApplyNamespaceRewriteWithAlias
#[test]
fn test_apply_namespace_rewrite_with_alias() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            import_aliases: string_map(&[("effect", "Fx")]),
            ..Default::default()
        }));

        let export = make_export("effect", "effect/Effect", "");
        let fix =
            Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect/Effect", "succeed"));

        let Some(result) = sp.apply(Some(&export), Some(fix)) else {
            t.fatal("expected non-nil result");
        };
        if result.import_kind != lsproto::ImportKind::Namespace {
            t.error(sprintf!("expected ImportKindNamespace, got %v", dbg(&result.import_kind)));
        }
        if result.name != "succeed" {
            t.error(sprintf!(
                "expected name 'succeed' (original export name), got %q",
                result.name
            ));
        }
        if result.namespace_prefix != "Fx" {
            t.error(sprintf!("expected namespace prefix 'Fx', got %q", result.namespace_prefix));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:178 TestApplyNamespaceReexport
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
                *unexpected_in_callback.lock() = Some(export.target.module_id.0.clone());
            }
            ("effect/Effect".to_string(), ResultKind::NodeModules)
        }));

        let mut export = make_export("effect", "effect", "effect/Effect");
        export.export_id.export_name = "Effect".to_string();
        export.syntax = ExportSyntax::Modifier;
        export.flags = SymbolFlags::ValueModule;
        let fix = Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect", "Effect"));

        let result = sp.apply(Some(&export), Some(fix));
        let unexpected_module_id = unexpected.lock().take();
        if let Some(module_id) = unexpected_module_id {
            t.fatal(sprintf!("unexpected target module: %q", module_id));
        }
        let Some(result) = result else {
            t.fatal("expected non-nil result");
        };
        if result.import_kind != lsproto::ImportKind::Namespace {
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

// go test: internal/autoimportstyle/stylepolicy_test.go:220 newBarrelPolicy
fn new_barrel_policy(options: &ResolvedEffectPluginOptions, disk: &[(&str, &str)]) -> StylePolicy {
    let mut sp = new_style_policy(Some(options));
    let disk = string_map(disk);
    sp.barrels = Some(Mutex::new(ets_importstyle::new_barrel_reader(
        |_| None,
        move |file_name| disk.get(file_name).cloned(),
    )));
    sp
}

// go test: internal/autoimportstyle/stylepolicy_test.go:232 makeBarrelFix
fn make_barrel_fix(module_specifier: &str, module_file_name: &str, name: &str) -> Fix {
    let mut fix = make_add_new_fix(lsproto::ImportKind::Named, module_specifier, name);
    fix.module_file_name = module_file_name.to_string();
    fix
}

// go test: internal/autoimportstyle/stylepolicy_test.go:238 TestApplyBarrelRewrite
#[test]
fn test_apply_barrel_rewrite() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["@effect/platform"]),
                ..Default::default()
            },
            &[(
                "/node_modules/@effect/platform/dist/dts/index.d.ts",
                r#"export * as HttpClient from "./HttpClient.js""#,
            )],
        );

        let export = make_export("@effect/platform", "@effect/platform/HttpClient", "");
        let fix = Arc::new(make_barrel_fix(
            "@effect/platform/HttpClient",
            "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
            "request",
        ));

        let Some(result) = sp.apply(Some(&export), Some(fix)) else {
            t.fatal("expected non-nil result");
        };
        if result.import_kind != lsproto::ImportKind::Named {
            t.error(sprintf!("expected ImportKindNamed, got %v", dbg(&result.import_kind)));
        }
        if result.module_specifier != "@effect/platform" {
            t.error(sprintf!(
                "expected module specifier '@effect/platform', got %q",
                result.module_specifier
            ));
        }
        if result.name != "request" {
            t.error(sprintf!(
                "expected name 'request' (original export name), got %q",
                result.name
            ));
        }
        if result.namespace_prefix != "HttpClient" {
            t.error(sprintf!(
                "expected namespace prefix 'HttpClient', got %q",
                result.namespace_prefix
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:265 TestApplyBarrelRewriteUsesNearestForwardingBarrel
#[test]
fn test_apply_barrel_rewrite_uses_nearest_forwarding_barrel() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["acme"]),
                ..Default::default()
            },
            &[
                (
                    "/node_modules/acme/dist/index.d.ts",
                    r#"export * as Http from "./HttpClient.ts""#,
                ),
                (
                    "/node_modules/acme/dist/forms/index.d.ts",
                    r#"export * as TextInput from "./TextInput.ts""#,
                ),
            ],
        );

        let tests: &[(&str, &str, &str, &str)] = &[
            ("acme/HttpClient", "/node_modules/acme/dist/HttpClient.d.ts", "acme", "Http"),
            (
                "acme/forms/TextInput",
                "/node_modules/acme/dist/forms/TextInput.d.ts",
                "acme/forms",
                "TextInput",
            ),
        ];
        for &(module_specifier, module_file_name, barrel, prefix) in tests {
            let result = sp.apply(
                Some(&make_export("acme", module_specifier, "")),
                Some(Arc::new(make_barrel_fix(module_specifier, module_file_name, "make"))),
            );
            let Some(result) = result else {
                t.fatal(sprintf!("%s: expected non-nil result", module_specifier));
            };
            if result.module_specifier != barrel {
                t.error(sprintf!(
                    "%s: expected module specifier %q, got %q",
                    module_specifier,
                    barrel,
                    result.module_specifier
                ));
            }
            if result.namespace_prefix != prefix {
                t.error(sprintf!(
                    "%s: expected namespace prefix %q, got %q",
                    module_specifier,
                    prefix,
                    result.namespace_prefix
                ));
            }
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:297 TestApplyBarrelRewriteWithoutBarrelPassthrough
#[test]
fn test_apply_barrel_rewrite_without_barrel_passthrough() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["acme"]),
                ..Default::default()
            },
            &[("/node_modules/acme/dist/index.d.ts", r#"export * as Button from "./Button.ts""#)],
        );

        let fix = Arc::new(make_barrel_fix(
            "acme/Private",
            "/node_modules/acme/dist/Private.d.ts",
            "secret",
        ));
        let result =
            sp.apply(Some(&make_export("acme", "acme/Private", "")), Some(Arc::clone(&fix)));
        if !result.as_ref().is_some_and(|result| Arc::ptr_eq(result, &fix)) {
            t.error(sprintf!(
                "expected fix for a module no barrel forwards to be returned unchanged, got %#v",
                dbg(&result)
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:309 TestApplyBarrelRewriteFromBarrelPassthrough
#[test]
fn test_apply_barrel_rewrite_from_barrel_passthrough() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["effect"]),
                ..Default::default()
            },
            &[(
                "/node_modules/effect/dist/index.d.ts",
                r#"export * as Function from "./Function.js""#,
            )],
        );

        for (module_specifier, module_file_name) in [
            ("effect", "/node_modules/effect/dist/index.d.ts"),
            ("effect/testing", "/node_modules/effect/dist/testing/index.d.ts"),
            ("effect/http", "/node_modules/effect/dist/http/index.d.ts"),
        ] {
            let mut export = make_export("effect", module_specifier, "effect/Function");
            export.syntax = ExportSyntax::Named;
            let fix = Arc::new(make_barrel_fix(module_specifier, module_file_name, "pipe"));
            let result = sp.apply(Some(&export), Some(Arc::clone(&fix)));
            if !result.as_ref().is_some_and(|result| Arc::ptr_eq(result, &fix)) {
                t.error(sprintf!(
                    "%s: expected fix from a barrel to be returned unchanged, got %#v",
                    module_specifier,
                    dbg(&result)
                ));
            }
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:329 TestApplyBarrelRewriteWithAlias
#[test]
fn test_apply_barrel_rewrite_with_alias() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["@effect/platform"]),
                import_aliases: string_map(&[("@effect/platform", "Platform")]),
                ..Default::default()
            },
            &[(
                "/node_modules/@effect/platform/dist/dts/index.d.ts",
                r#"export * as HttpClient from "./HttpClient.js""#,
            )],
        );

        let export = make_export("@effect/platform", "@effect/platform/HttpClient", "");
        let fix = Arc::new(make_barrel_fix(
            "@effect/platform/HttpClient",
            "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
            "request",
        ));

        let Some(result) = sp.apply(Some(&export), Some(fix)) else {
            t.fatal("expected non-nil result");
        };
        if result.name != "request" {
            t.error(sprintf!(
                "expected name 'request' (original export name), got %q",
                result.name
            ));
        }
        if result.namespace_prefix != "Platform" {
            t.error(sprintf!(
                "expected namespace prefix 'Platform', got %q",
                result.namespace_prefix
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:351 TestApplyNamespaceRewriteWithoutUsagePositionFallsBack
#[test]
fn test_apply_namespace_rewrite_without_usage_position_falls_back() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let export = make_export("effect", "effect/Effect", "");
        let mut fix = make_add_new_fix(lsproto::ImportKind::Named, "effect/Effect", "succeed");
        fix.auto_import_fix.usage_position = None;
        let fix = Arc::new(fix);

        let result = sp.apply(Some(&export), Some(Arc::clone(&fix)));
        if !result.is_some_and(|result| Arc::ptr_eq(&result, &fix)) {
            t.error("expected unchanged fix when usage position is unavailable");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:367 TestApplyBarrelRewriteWithoutUsagePositionFallsBack
#[test]
fn test_apply_barrel_rewrite_without_usage_position_falls_back() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_barrel_policy(
            &ResolvedEffectPluginOptions {
                barrel_import_packages: strings(&["@effect/platform"]),
                ..Default::default()
            },
            &[(
                "/node_modules/@effect/platform/dist/dts/index.d.ts",
                r#"export * as HttpClient from "./HttpClient.js""#,
            )],
        );

        let export = make_export("@effect/platform", "@effect/platform/HttpClient", "");
        let mut fix = make_barrel_fix(
            "@effect/platform/HttpClient",
            "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
            "request",
        );
        fix.auto_import_fix.usage_position = None;
        let fix = Arc::new(fix);

        let result = sp.apply(Some(&export), Some(Arc::clone(&fix)));
        if !result.is_some_and(|result| Arc::ptr_eq(&result, &fix)) {
            t.error("expected unchanged fix when usage position is unavailable");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:383 TestApplyTopLevelReexportIgnore
#[test]
fn test_apply_top_level_reexport_ignore() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            top_level_named_reexports: TopLevelNamedReexportsMode::Ignore, // "ignore"
            ..Default::default()
        }));

        // Export is a reexport (target module differs from module)
        let mut export = make_export("effect", "effect", "effect/Effect");
        export.syntax = ExportSyntax::Named;
        let fix = Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect", "succeed"));

        let Some(result) = sp.apply(Some(&export), Some(fix)) else {
            t.fatal("expected non-nil result");
        };
        // When ignoring reexports, the fix should be returned unchanged
        if result.import_kind != lsproto::ImportKind::Named {
            t.error(sprintf!(
                "expected ImportKindNamed (unchanged), got %v",
                dbg(&result.import_kind)
            ));
        }
        if result.name != "succeed" {
            t.error(sprintf!("expected name 'succeed' (unchanged), got %q", result.name));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:409 TestApplyTopLevelReexportFollow
#[test]
fn test_apply_top_level_reexport_follow() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            top_level_named_reexports: TopLevelNamedReexportsMode::Follow, // "follow"
            ..Default::default()
        }));

        // Export is a reexport (target module differs from module)
        let mut export = make_export("effect", "effect", "effect/Effect");
        export.syntax = ExportSyntax::Named;
        let fix = Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect", "succeed"));

        let result = sp.apply(Some(&export), Some(fix));
        // When following reexports, the reexport fix should be suppressed (nil)
        // so the direct submodule namespace import wins instead
        if let Some(result) = result {
            t.error(sprintf!(
                "expected nil result (suppressed reexport), got fix with ImportKind=%v",
                dbg(&result.import_kind)
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:429 TestApplyNoMatchPassthrough
#[test]
fn test_apply_no_match_passthrough() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let export = make_export("lodash", "lodash", "");
        let fix = Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "lodash", "map"));

        let result = sp.apply(Some(&export), Some(Arc::clone(&fix)));
        if !result.is_some_and(|result| Arc::ptr_eq(&result, &fix)) {
            t.error("expected fix to be returned unchanged (same pointer)");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:444 TestApplyNonAddNewPassthrough
#[test]
fn test_apply_non_add_new_passthrough() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let export = make_export("effect", "effect/Effect", "");
        let fix = Arc::new(Fix {
            auto_import_fix: lsproto::AutoImportFix {
                kind: lsproto::AutoImportFixKind::AddToExisting,
                import_kind: lsproto::ImportKind::Named,
                module_specifier: "effect/Effect".to_string(),
                name: "succeed".to_string(),
                ..Default::default()
            },
            ..Default::default()
        });

        let result = sp.apply(Some(&export), Some(Arc::clone(&fix)));
        if !result.is_some_and(|result| Arc::ptr_eq(&result, &fix)) {
            t.error("expected non-AddNew fix to be returned unchanged");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:466 TestApplyCaseInsensitiveMatching
#[test]
fn test_apply_case_insensitive_matching() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["Effect"]),
            ..Default::default()
        }));

        // Package name is lowercase but config was uppercase
        let export = make_export("effect", "effect/Effect", "");
        let fix =
            Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect/Effect", "succeed"));

        let Some(result) = sp.apply(Some(&export), Some(fix)) else {
            t.fatal("expected non-nil result");
        };
        if result.import_kind != lsproto::ImportKind::Namespace {
            t.error(sprintf!(
                "expected case-insensitive match to trigger namespace rewrite, got %v",
                dbg(&result.import_kind)
            ));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:484 TestApplyNilInputs
#[test]
fn test_apply_nil_inputs() {
    ets_go::testing::run(|t| {
        t.parallel();
        let sp = new_style_policy(Some(&ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["effect"]),
            ..Default::default()
        }));

        let fix =
            Arc::new(make_add_new_fix(lsproto::ImportKind::Named, "effect/Effect", "succeed"));
        if sp.apply(None, Some(fix)).is_none() {
            t.error("expected fix returned when export is nil");
        }

        let export = make_export("effect", "effect/Effect", "");
        if sp.apply(Some(&export), None).is_some() {
            t.error("expected nil when fix is nil");
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:500 TestNewFixTransformerNilForEmptyPrefs
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

// go test: internal/autoimportstyle/stylepolicy_test.go:508 TestNewFixTransformerAppliesPolicy
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
            lsproto::ImportKind::Named,
            "effect/Effect",
            "succeed",
        ))];

        let result = transformer(Some(&export), fixes);
        if result.len() != 1 {
            t.fatal(sprintf!("expected 1 fix, got %d", result.len()));
        }
        if result[0].import_kind != lsproto::ImportKind::Namespace {
            t.error(sprintf!("expected ImportKindNamespace, got %v", dbg(&result[0].import_kind)));
        }
    });
}

// go test: internal/autoimportstyle/stylepolicy_test.go:529 TestNewFixTransformerPrefersExistingNamespaceUse
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
                kind: lsproto::AutoImportFixKind::UseNamespace,
                import_kind: lsproto::ImportKind::Namespace,
                module_specifier: "effect/testing/TestClock".to_string(),
                name: "testClockWith".to_string(),
                namespace_prefix: "TestClock".to_string(),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut add_to_existing = make_add_new_fix(
            lsproto::ImportKind::Named,
            "effect/testing/TestClock",
            "testClockWith",
        );
        add_to_existing.auto_import_fix.kind = lsproto::AutoImportFixKind::AddToExisting;

        let result = transformer(Some(&export), vec![use_namespace, Arc::new(add_to_existing)]);
        if result.len() != 1 {
            t.fatal(sprintf!("expected 1 fix, got %d", result.len()));
        }
        if result[0].kind != lsproto::AutoImportFixKind::UseNamespace {
            t.fatal(sprintf!("expected UseNamespace fix, got %v", dbg(&result[0].kind)));
        }
    });
}
