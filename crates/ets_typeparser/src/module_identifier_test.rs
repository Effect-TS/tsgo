use ets_go::sprintf;
use ets_shim::ast::{SourceFile, SourceFileParseOptions};
use ets_shim::core::{P, ScriptKind};

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/module_identifier_test.go:11 parseSource
// Rust: the parsed file is unbound and outside any program, as Go's.
pub(crate) fn parse_source(source: &str) -> P<SourceFile> {
    ets_shim::parser::parse_source_file(
        SourceFileParseOptions { file_name: "/test.ts".to_string(), ..Default::default() },
        source,
        ScriptKind::TS,
    )
}

// go test: internal/typeparser/module_identifier_test.go:17 TestFindModuleIdentifierForPackage_NamespaceImport
#[test]
fn test_find_module_identifier_for_package_namespace_import() {
    run(|t| {
        t.parallel();
        let source = r#"import * as SchemaModules from "effect/schema""#;
        let sf = parse_source(source);

        let got = find_module_identifier_for_package(sf, "effect", "schema");
        if got != "SchemaModules" {
            t.error(sprintf!("FindModuleIdentifierForPackage = %q, want %q", got, "SchemaModules"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:28 TestFindModuleIdentifierForPackage_NamedImportWithAlias
#[test]
fn test_find_module_identifier_for_package_named_import_with_alias() {
    run(|t| {
        t.parallel();
        let source = r#"import { Model as M } from "effect/schema""#;
        let sf = parse_source(source);

        let got = find_module_identifier_for_package(sf, "effect/schema", "Model");
        if got != "M" {
            t.error(sprintf!("FindModuleIdentifierForPackage = %q, want %q", got, "M"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:39 TestFindModuleIdentifierForPackage_NamedImportNoAlias
#[test]
fn test_find_module_identifier_for_package_named_import_no_alias() {
    run(|t| {
        t.parallel();
        let source = r#"import { Model } from "effect/schema""#;
        let sf = parse_source(source);

        let got = find_module_identifier_for_package(sf, "effect/schema", "Model");
        if got != "Model" {
            t.error(sprintf!("FindModuleIdentifierForPackage = %q, want %q", got, "Model"));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:50 TestFindModuleIdentifierForPackage_NoMatchingImport
#[test]
fn test_find_module_identifier_for_package_no_matching_import() {
    run(|t| {
        t.parallel();
        let source = r#"import { Schema } from "effect""#;
        let sf = parse_source(source);

        let got = find_module_identifier_for_package(sf, "effect/schema", "Model");
        if got != "Model" {
            t.error(sprintf!(
                "FindModuleIdentifierForPackage = %q, want fallback %q",
                got,
                "Model"
            ));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:61 TestFindModuleIdentifierForPackage_NilSourceFile
#[test]
fn test_find_module_identifier_for_package_nil_source_file() {
    run(|t| {
        t.parallel();
        let got = find_module_identifier_for_package(None, "effect/schema", "Model");
        if got != "Model" {
            t.error(sprintf!(
                "FindModuleIdentifierForPackage = %q, want fallback %q",
                got,
                "Model"
            ));
        }
    });
}

// go test: internal/typeparser/module_identifier_test.go:69 TestFindModuleIdentifier_ExistingBehavior
#[test]
fn test_find_module_identifier_existing_behavior() {
    run(|t| {
        t.parallel();

        t.run("namespace import", |t| {
            t.parallel();
            let source = r#"import * as S from "effect/Schema""#;
            let sf = parse_source(source);
            let got = find_module_identifier(sf, "Schema");
            if got != "S" {
                t.error(sprintf!("FindModuleIdentifier = %q, want %q", got, "S"));
            }
        });

        t.run("named import with alias", |t| {
            t.parallel();
            let source = r#"import { Schema as Sc } from "effect""#;
            let sf = parse_source(source);
            let got = find_module_identifier(sf, "Schema");
            if got != "Sc" {
                t.error(sprintf!("FindModuleIdentifier = %q, want %q", got, "Sc"));
            }
        });

        t.run("named import no alias", |t| {
            t.parallel();
            let source = r#"import { Schema } from "effect""#;
            let sf = parse_source(source);
            let got = find_module_identifier(sf, "Schema");
            if got != "Schema" {
                t.error(sprintf!("FindModuleIdentifier = %q, want %q", got, "Schema"));
            }
        });

        t.run("fallback", |t| {
            t.parallel();
            let source = "const x = 1";
            let sf = parse_source(source);
            let got = find_module_identifier(sf, "Schema");
            if got != "Schema" {
                t.error(sprintf!("FindModuleIdentifier = %q, want fallback %q", got, "Schema"));
            }
        });
    });
}
