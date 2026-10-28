// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_fourslash::FourslashTest;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::lsconv;
use ets_shim::lsproto::DocumentUri;
use libtest_mimic::Trial;

use crate::{has_diagnostic_code, run_case};

/// Rust-only: the Go test's local `fixCase` struct.
struct FixCase {
    name: &'static str,
    source: String,
    expected: String,
    matches: i32,
    fixes: i32,
    // unresolved marks fixes that intentionally reference an unbound
    // Schema; only their text is checked.
    unresolved: bool,
}

/// Rust-only: a fixture case of the Go test (`fixCase{name: ..., matches: ..., fixes: ...}`).
fn fixture(name: &'static str, matches: i32, fixes: i32) -> FixCase {
    FixCase {
        name,
        source: String::new(),
        expected: String::new(),
        matches,
        fixes,
        unresolved: false,
    }
}

/// Rust-only: the Go test's `countMatches` closure.
fn count_matches(t: &T, f: &FourslashTest, uri: &DocumentUri, code: i32) -> i32 {
    let mut count = 0;
    for diagnostic in f.get_quick_fixes_for_diagnostics(t, uri) {
        if has_diagnostic_code(&diagnostic.diagnostic, code) {
            count += 1;
        }
    }
    count
}

// go test: internal/effecttest/type_union_to_schema_union_test.go:17 TestTypeUnionToSchemaUnionFixesCompile 5686519354923351
fn test_type_union_to_schema_union_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
            const PREFIX: &str = r#"// @filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": { "*": "off", "typeUnionToSchemaUnion": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            // union spells Schema.Union arguments for the version under test.
            let union = |args: &str| -> String {
                if version == EffectVersion::V4 {
                    return format!("[{args}]");
                }
                args.to_string()
            };
            let mut cases = vec![
                FixCase {
                    name: "exported",
                    source: r#"import { Schema } from "effect"
export const A = Schema.Struct({ a: Schema.String })
export const B = Schema.Struct({ b: Schema.String })
export type AB = typeof A.Type | typeof B.Type
"#
                    .to_string(),
                    expected: r#"import { Schema } from "effect"
export const A = Schema.Struct({ a: Schema.String })
export const B = Schema.Struct({ b: Schema.String })
export const AB = Schema.Union("#
                        .to_string()
                        + &union("A, B")
                        + r");
export type AB = typeof AB.Type
",
                    matches: 1,
                    fixes: 1,
                    unresolved: false,
                },
                FixCase {
                    // The const goes before the JSDoc, which stays attached to the alias.
                    name: "jsDoc",
                    source: r#"import * as S from "effect/Schema"
const A = S.String
const B = S.Boolean
/** Either A or B */
type AB = typeof A.Encoded | typeof B.Encoded
export const value: AB = "a"
"#
                    .to_string(),
                    expected: r#"import * as S from "effect/Schema"
const A = S.String
const B = S.Boolean
const AB = S.Union("#
                        .to_string()
                        + &union("A, B")
                        + r#");
/** Either A or B */
type AB = typeof AB.Encoded
export const value: AB = "a"
"#,
                    matches: 1,
                    fixes: 1,
                    unresolved: false,
                },
                FixCase {
                    name: "namespace",
                    source: r#"import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export namespace Models {
  export type AB = typeof A.Type | typeof B.Type
}
"#
                    .to_string(),
                    expected: r#"import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export namespace Models {
  export const AB = Schema.Union("#
                        .to_string()
                        + &union("A, B")
                        + r");
  export type AB = typeof AB.Type
}
",
                    matches: 1,
                    fixes: 1,
                    unresolved: false,
                },
                FixCase {
                    // Code before the alias on its line is not indentation: the const
                    // is inserted inline instead of repeating `function f() { `.
                    name: "inlineFunctionBody",
                    source: r#"import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export function f() { type AB = typeof A.Type | typeof B.Type; return [] as Array<AB> }
"#
                    .to_string(),
                    expected: r#"import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export function f() { const AB = Schema.Union("#
                        .to_string()
                        + &union("A, B")
                        + r"); type AB = typeof AB.Type; return [] as Array<AB> }
",
                    matches: 1,
                    fixes: 1,
                    unresolved: false,
                },
                FixCase {
                    name: "sameLineStatement",
                    source: r#"import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export const n = 1; export type AB = typeof A.Encoded | typeof B.Encoded
"#
                    .to_string(),
                    expected: r#"import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export const n = 1; export const AB = Schema.Union("#
                        .to_string()
                        + &union("A, B")
                        + r"); export type AB = typeof AB.Encoded
",
                    matches: 1,
                    fixes: 1,
                    unresolved: false,
                },
                FixCase {
                    // A const inserted before the alias would use B before its declaration.
                    name: "lateMember",
                    source: r#"import { Schema } from "effect"
const A = Schema.String
export type AB = typeof A.Type | typeof B.Type
const B = Schema.Boolean
export const value: AB = "a"
"#
                    .to_string(),
                    expected: String::new(),
                    matches: 1,
                    fixes: 0,
                    unresolved: false,
                },
                FixCase {
                    // A qualified member whose namespace is declared later is late too.
                    name: "lateNamespace",
                    source: r#"import { Schema } from "effect"
export type AB = typeof Models.A.Type | typeof Models.B.Type
namespace Models {
  export const A = Schema.String
  export const B = Schema.Boolean
}
export const value: AB = "a"
"#
                    .to_string(),
                    expected: String::new(),
                    matches: 1,
                    fixes: 0,
                    unresolved: false,
                },
                FixCase {
                    // No Schema module binding: the fix falls back to Schema and adds no import.
                    name: "fallbackReference",
                    source: r#"import { Boolean as SchemaBoolean, String as SchemaString } from "effect/Schema"
export type AB = typeof SchemaString.Type | typeof SchemaBoolean.Type
"#
                    .to_string(),
                    expected: r#"import { Boolean as SchemaBoolean, String as SchemaString } from "effect/Schema"
export const AB = Schema.Union("#
                        .to_string()
                        + &union("SchemaString, SchemaBoolean")
                        + r");
export type AB = typeof AB.Type
",
                    matches: 1,
                    fixes: 1,
                    unresolved: true,
                },
            ];
            if version == EffectVersion::V4 {
                cases.extend([
                    fixture("typeUnionToSchemaUnion.ts", 23, 21),
                    fixture("typeUnionToSchemaUnion_aliasedImport.ts", 4, 4),
                ]);
            } else {
                cases.extend([
                    fixture("typeUnionToSchemaUnion.ts", 23, 21),
                    fixture("typeUnionToSchemaUnion_aliasedImport.ts", 3, 3),
                ]);
            }
            cases.push(fixture("typeUnionToSchemaUnion_preview.ts", 1, 1));

            let code = tsdiag::THIS_TYPE_ALIAS_UNIONS_SCHEMA_0_TYPES_WITHOUT_A_RUNTIME_SCHEMA_COMPOSE_THEM_WITH_SCHEMA_UNION_AND_DERIVE_THE_TYPE_FROM_IT_EFFECT_TYPE_UNION_TO_SCHEMA_UNION.code();
            let uri = lsconv::file_name_to_document_uri("/repro.ts");
            for test in &cases {
                t.run(test.name, |t| {
                    let mut source = test.source.clone();
                    if source.is_empty() {
                        let fixture = match std::fs::read_to_string(
                            ets_effecttest::test_data_path()
                                .join("tests")
                                .join(version.as_str())
                                .join(test.name),
                        ) {
                            Ok(fixture) => fixture,
                            Err(err) => t.fatal(err.to_string()),
                        };
                        // The fixture enables the rule as a warning; keep it a suggestion so
                        // VerifyNonSuggestionDiagnostics only sees compile errors.
                        source = fixture.replacen(
                            "typeUnionToSchemaUnion:warning",
                            "typeUnionToSchemaUnion:suggestion",
                            1,
                        );
                    }
                    let version_line = format!("// @{}\n", version.as_str());
                    if !source.starts_with(&version_line) {
                        source = format!("{version_line}{source}");
                    }
                    let mut f = ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{source}"));
                    f.go_to_file(t, "/repro.ts");
                    f.verify_non_suggestion_diagnostics(t, &[]);
                    let (mut matches, mut fixes) = (0, 0);
                    for diagnostic in &f.get_quick_fixes_for_diagnostics(t, &uri) {
                        if !has_diagnostic_code(&diagnostic.diagnostic, code) {
                            continue;
                        }
                        matches += 1;
                        for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                            if title != "Convert to Schema.Union" {
                                continue;
                            }
                            let result = f.apply_quick_fix(t, &uri, &diagnostic.diagnostic, index);
                            if result.changes.len() != 1 {
                                t.fatal(format!(
                                    "expected one changed file, got {}",
                                    result.changes.len()
                                ));
                            }
                            let after = &result.changes[0].after;
                            let want = format!("{version_line}{}", test.expected);
                            if !test.expected.is_empty() && *after != want {
                                t.fatal(format!("replacement = {after:?}, want {want:?}"));
                            }
                            if !test.unresolved {
                                let mut converted = ets_fourslash::new_fourslash(
                                    t,
                                    None,
                                    &format!("{PREFIX}{after}"),
                                );
                                converted.go_to_file(t, "/repro.ts");
                                converted.verify_non_suggestion_diagnostics(t, &[]);
                                let remaining = count_matches(t, &converted, &uri, code);
                                if remaining != test.matches - 1 {
                                    t.fatal(format!(
                                        "expected {} diagnostics after replacement, got {remaining}",
                                        test.matches - 1
                                    ));
                                }
                                converted.done(t);
                            }
                            fixes += 1;
                        }
                    }
                    if matches != test.matches || fixes != test.fixes {
                        t.fatal(format!(
                            "got {matches} diagnostics and {fixes} fixes, want {} and {}",
                            test.matches, test.fixes
                        ));
                    }
                    f.done(t);
                });
            }
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestTypeUnionToSchemaUnionFixesCompile".to_string(),
        test_type_union_to_schema_union_fixes_compile,
    )]
}
