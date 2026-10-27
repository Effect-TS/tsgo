use ets_go::sprintf;
use ets_shim::ast::{Node, SourceFile, SourceFileParseOptions};
use ets_shim::core::{P, ScriptKind};

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/module_identifier_test.go:12 parseSource
// Rust: the parsed file is unbound and outside any program, as Go's.
pub(crate) fn parse_source(source: &str) -> P<SourceFile> {
    ets_shim::parser::parse_source_file(
        SourceFileParseOptions { file_name: "/test.ts".to_string(), ..Default::default() },
        source,
        ScriptKind::TS,
    )
}

// go test: internal/typeparser/module_identifier_test.go:18 TestFindModuleReferenceForPackage_NamespaceImport
#[test]
fn test_find_module_reference_for_package_namespace_import() {
    run(|t| {
        t.parallel();
        let source = r#"import * as SchemaModules from "effect/schema""#;
        let sf = parse_source(source);

        let got = find_module_reference_for_package(sf, "effect", "schema").string();
        if got != "SchemaModules" {
            t.error(sprintf!("FindModuleReferenceForPackage = %q, want %q", got, "SchemaModules"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:29 TestFindModuleReferenceForPackage_NamedImportWithAlias
#[test]
fn test_find_module_reference_for_package_named_import_with_alias() {
    run(|t| {
        t.parallel();
        let source = r#"import { Model as M } from "effect/schema""#;
        let sf = parse_source(source);

        let got = find_module_reference_for_package(sf, "effect/schema", "Model").string();
        if got != "M" {
            t.error(sprintf!("FindModuleReferenceForPackage = %q, want %q", got, "M"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:40 TestFindModuleReferenceForPackage_NamedImportNoAlias
#[test]
fn test_find_module_reference_for_package_named_import_no_alias() {
    run(|t| {
        t.parallel();
        let source = r#"import { Model } from "effect/schema""#;
        let sf = parse_source(source);

        let got = find_module_reference_for_package(sf, "effect/schema", "Model").string();
        if got != "Model" {
            t.error(sprintf!("FindModuleReferenceForPackage = %q, want %q", got, "Model"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:51 TestFindModuleReferenceForPackage_NoMatchingImport
#[test]
fn test_find_module_reference_for_package_no_matching_import() {
    run(|t| {
        t.parallel();
        let source = r#"import { Schema } from "effect""#;
        let sf = parse_source(source);

        let got = find_module_reference_for_package(sf, "effect/schema", "Model").string();
        if got != "Model" {
            t.error(sprintf!("FindModuleReferenceForPackage = %q, want fallback %q", got, "Model"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:62 TestFindModuleReferenceForPackage_NilSourceFile
#[test]
fn test_find_module_reference_for_package_nil_source_file() {
    run(|t| {
        t.parallel();
        let got = find_module_reference_for_package(None, "effect/schema", "Model").string();
        if got != "Model" {
            t.error(sprintf!("FindModuleReferenceForPackage = %q, want fallback %q", got, "Model"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:70 TestFindModuleReference_ExistingBehavior
#[test]
fn test_find_module_reference_existing_behavior() {
    run(|t| {
        t.parallel();

        t.run("namespace import", |t| {
            t.parallel();
            let source = r#"import * as S from "effect/Schema""#;
            let sf = parse_source(source);
            let got = find_module_reference(sf, "Schema").string();
            if got != "S" {
                t.error(sprintf!("FindModuleReference = %q, want %q", got, "S"));
            }
        });

        t.run("named import with alias", |t| {
            t.parallel();
            let source = r#"import { Schema as Sc } from "effect""#;
            let sf = parse_source(source);
            let got = find_module_reference(sf, "Schema").string();
            if got != "Sc" {
                t.error(sprintf!("FindModuleReference = %q, want %q", got, "Sc"));
            }
        });

        t.run("named import no alias", |t| {
            t.parallel();
            let source = r#"import { Schema } from "effect""#;
            let sf = parse_source(source);
            let got = find_module_reference(sf, "Schema").string();
            if got != "Schema" {
                t.error(sprintf!("FindModuleReference = %q, want %q", got, "Schema"));
            }
        });

        t.run("fallback", |t| {
            t.parallel();
            let source = "const x = 1";
            let sf = parse_source(source);
            let got = find_module_reference(sf, "Schema").string();
            if got != "Schema" {
                t.error(sprintf!("FindModuleReference = %q, want fallback %q", got, "Schema"));
            }
        });
    });
}

// go test: internal/typeparser/module_identifier_test.go:114 TestFindModuleReference_BarrelNamespaceImport
#[test]
fn test_find_module_reference_barrel_namespace_import() {
    run(|t| {
        t.parallel();
        let cases: [(&str, &str, &str, &str, &str); 6] = [
            ("effect barrel", r#"import * as X from "effect""#, "effect", "Effect", "X.Effect"),
            (
                "effect barrel other module",
                r#"import * as X from "effect""#,
                "effect",
                "Layer",
                "X.Layer",
            ),
            (
                "schema barrel",
                r#"import * as S from "effect/schema""#,
                "effect/schema",
                "Model",
                "S.Model",
            ),
            (
                "direct import wins over barrel",
                "import * as X from \"effect\"\nimport { Effect as E } from \"effect\"",
                "effect",
                "Effect",
                "E",
            ),
            (
                "module namespace wins over barrel",
                "import * as X from \"effect\"\nimport * as Eff from \"effect/Effect\"",
                "effect",
                "Effect",
                "Eff",
            ),
            (
                "other package barrel ignored",
                r#"import * as X from "effectful""#,
                "effect",
                "Effect",
                "Effect",
            ),
        ];
        for (name, source, pkg, module, want) in cases {
            t.run(name, |t| {
                t.parallel();
                let got = find_module_reference_for_package(parse_source(source), pkg, module);
                if got.string() != want {
                    t.error(sprintf!(
                        "FindModuleReferenceForPackage = %q, want %q",
                        got.string(),
                        want
                    ));
                }
            });
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:137 TestFindEffectModuleReference
#[test]
fn test_find_effect_module_reference() {
    run(|t| {
        t.parallel();
        let cases: [(&str, &str, &[&str]); 9] = [
            ("named import", r#"import { Effect } from "effect""#, &["Effect"]),
            ("named import with alias", r#"import { Effect as E } from "effect""#, &["E"]),
            ("module namespace import", r#"import * as T from "effect/Effect""#, &["T"]),
            ("barrel namespace import", r#"import * as X from "effect""#, &["X", "Effect"]),
            (
                "named function import falls back",
                r#"import { flatMap } from "effect/Effect""#,
                &["Effect"],
            ),
            ("fallback", "const x = 1", &["Effect"]),
            (
                "type-only named import skipped",
                "import type { Effect } from \"effect\"\nimport * as X from \"effect\"",
                &["X", "Effect"],
            ),
            (
                "type-only specifier skipped",
                r#"import { type Effect, Layer } from "effect""#,
                &["Effect"],
            ),
            (
                "type-only namespace import skipped",
                r#"import type * as T from "effect/Effect""#,
                &["Effect"],
            ),
        ];
        for (name, source, want) in cases {
            t.run(name, |t| {
                t.parallel();
                let got = find_effect_module_reference(parse_source(source));
                let want = ModuleReference::of(want);
                if got != want {
                    t.error(format!("FindEffectModuleReference = {:?}, want {:?}", got.0, want.0));
                }
            });
        }
        let got = find_effect_module_reference(None);
        if got != ModuleReference::of(&["Effect"]) {
            t.error(format!("FindEffectModuleReference(nil) = {:?}, want fallback", got.0));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:167 TestModuleReferenceMatchesNode
#[test]
fn test_module_reference_matches_node() {
    run(|t| {
        t.parallel();
        let sf = parse_source("X.Effect\nEffect\nY.Effect\nX.Effect.map\nlet a: X.Effect");
        let expression = |i: usize| -> P<Node> {
            sf.statements().nodes()[i].as_expression_statement().expression()
        };
        let reference = ModuleReference::of(&["X", "Effect"]);
        if !reference.matches_node(expression(0)) {
            t.error("X.Effect should match");
        }
        if reference.matches_node(expression(1))
            || reference.matches_node(expression(2))
            || reference.matches_node(expression(3))
        {
            t.error("Effect, Y.Effect and X.Effect.map should not match");
        }
        if !ModuleReference::of(&["Effect"]).matches_node(expression(1)) {
            t.error("Effect should match");
        }
        let type_name = sf.statements().nodes()[4]
            .as_variable_statement()
            .declaration_list()
            .as_variable_declaration_list()
            .declarations()
            .nodes()[0]
            .as_variable_declaration()
            .type_()
            .expect("the type annotation")
            .as_type_reference_node()
            .type_name();
        if !reference.matches_node(type_name) {
            t.error("qualified name X.Effect should match");
        }
    });
}
