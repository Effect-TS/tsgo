// Ported from effect-tsgo's internal/typeparser/expected_and_real_type_test.go (package typeparser_test; see
// helpers_test.rs). Rust: `ExpectedAndRealType`'s `Node()` and `ValueNode()` are never nil here (`P<Node>`), so
// Go's nil checks on them hold trivially and are left out; a nil `*TypeParser` receiver cannot be written either,
// so the "nil checker" halves of the NilInputs tests are left out (see each test).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, SourceFile};
use ets_shim::core::P;
use ets_shim::vfs::vfstest::MapFile;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/expected_and_real_type_test.go:21 compileAndGetCheckerAndSourceFile c55fb678e808cbf1
// compileAndGetCheckerAndSourceFile compiles a TypeScript source string and returns the
// checker, type parser, source file, and a done function to release the checker.
//
// Rust: returns the `Compiled` the parser borrows (`compiled.type_parser()`); dropping it is Go's `done`.
pub(crate) fn compile_and_get_checker_and_source_file(
    t: &T,
    source: &str,
) -> (Compiled, P<SourceFile>) {
    t.helper();

    let mut testfs = TestFS::default();
    testfs.insert("/.src/test.ts".to_string(), MapFile::from(source));

    let compiled = new_compiled(testfs, test_compiler_options(), vec!["/.src/test.ts".to_string()]);
    let Some(sf) = compiled.program.get_source_file("/.src/test.ts") else {
        t.fatal("Failed to get source file");
    };
    (compiled, sf)
}

/// Rust-only: Go's `results.Len()` (a nil slice has length 0).
fn results_len(results: Option<&[ExpectedAndRealType]>) -> i64 {
    results.map_or(0, |results| results.len() as i64)
}

// go test: internal/typeparser/expected_and_real_type_test.go:61 TestExpectedAndRealTypes_VariableDeclaration c80daf110fc10948
#[test]
fn test_expected_and_real_types_variable_declaration() {
    run(|t| {
        t.parallel();

        let source = r#"
const a: number = 42
const b: string = "hello"
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find 2 variable declaration sites
        let mut count = 0_i64;
        for r in results.as_deref().unwrap_or_default() {
            if r.expected_type(&mut tp).is_some() && r.real_type(&mut tp).is_some() {
                count += 1;
            }
        }
        if count < 2 {
            t.error(sprintf!(
                "expected at least 2 assignment sites for variable declarations, got %d (total results: %d)",
                count,
                results_len(results.as_deref())
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:86 TestExpectedAndRealTypes_CallExpression 99f28d48e5222f18
#[test]
fn test_expected_and_real_types_call_expression() {
    run(|t| {
        t.parallel();

        let source = r#"
function foo(x: number, y: string): void {}
foo(42, "hello")
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find at least 2 call argument sites (for x and y)
        let mut count = 0_i64;
        for r in results.as_deref().unwrap_or_default() {
            if r.expected_type(&mut tp).is_some() && r.real_type(&mut tp).is_some() {
                count += 1;
            }
        }
        if count < 2 {
            t.error(sprintf!(
                "expected at least 2 assignment sites for call arguments, got %d (total results: %d)",
                count,
                results_len(results.as_deref())
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:111 TestExpectedAndRealTypes_ObjectLiteralProperty 7b200b73ebcda29c
#[test]
fn test_expected_and_real_types_object_literal_property() {
    run(|t| {
        t.parallel();

        let source = r#"
interface Obj { a: number; b: string }
const obj: Obj = { a: 1, b: "hello" }
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find sites for the variable declaration and the object literal properties
        if results_len(results.as_deref()) == 0 {
            t.error("expected at least one assignment site for object literal, got 0");
        }

        // Verify all results have non-nil types
        for (i, r) in results.as_deref().unwrap_or_default().iter().enumerate() {
            if r.expected_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: ExpectedType is nil", i as i64));
            }
            if r.real_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: RealType is nil", i as i64));
            }
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:140 TestExpectedAndRealTypes_BinaryAssignment be5430c2a62998fa
#[test]
fn test_expected_and_real_types_binary_assignment() {
    run(|t| {
        t.parallel();

        let source = r"
let x: number = 0
x = 42
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find at least 2 sites: variable declaration + binary assignment
        if results_len(results.as_deref()) < 2 {
            t.error(sprintf!(
                "expected at least 2 assignment sites (var decl + binary assign), got %d",
                results_len(results.as_deref())
            ));
        }

        // Verify all results have non-nil fields
        for (i, r) in results.as_deref().unwrap_or_default().iter().enumerate() {
            if r.expected_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: ExpectedType is nil", i as i64));
            }
            if r.real_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: RealType is nil", i as i64));
            }
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:172 TestExpectedAndRealTypes_ReturnStatement 567e23adcaab24da
#[test]
fn test_expected_and_real_types_return_statement() {
    run(|t| {
        t.parallel();

        let source = r"
function getNum(): number {
  return 42
}
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find at least 1 site for the return statement
        let mut found = false;
        for r in results.as_deref().unwrap_or_default() {
            if r.node().kind() == Kind::ReturnStatement {
                found = true;
                if r.expected_type(&mut tp).is_none() {
                    t.error("return statement: ExpectedType is nil");
                }
                if r.real_type(&mut tp).is_none() {
                    t.error("return statement: RealType is nil");
                }
            }
        }
        if !found {
            t.error(sprintf!(
                "expected to find a return statement assignment site, got %d total results",
                results_len(results.as_deref())
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:204 TestExpectedAndRealTypes_ArrowFunctionBodyNoTypeParams 4330b3a0e0479e5c
#[test]
fn test_expected_and_real_types_arrow_function_body_no_type_params() {
    run(|t| {
        t.parallel();

        let source = r"
const fn: () => number = () => 42
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find at least 1 site for the variable declaration, and potentially 1 for the arrow body
        if results_len(results.as_deref()) == 0 {
            t.error("expected at least 1 assignment site for arrow function body, got 0");
        }

        // Verify all results have valid types
        for (i, r) in results.as_deref().unwrap_or_default().iter().enumerate() {
            if r.expected_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: ExpectedType is nil", i as i64));
            }
            if r.real_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: RealType is nil", i as i64));
            }
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:232 TestExpectedAndRealTypes_ArrowFunctionBodyWithTypeParams 6b40071287899a24
#[test]
fn test_expected_and_real_types_arrow_function_body_with_type_params() {
    run(|t| {
        t.parallel();

        let source = r"
function wrap<T>(fn: () => T): T { return fn() }
const result = wrap(<A extends number>(): A => 42 as any as A)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find at least some assignment sites
        if results_len(results.as_deref()) == 0 {
            t.error(
                "expected at least 1 assignment site for arrow function with type params, got 0",
            );
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:251 TestExpectedAndRealTypes_SatisfiesExpression ea79c3da86d4d8ea
#[test]
fn test_expected_and_real_types_satisfies_expression() {
    run(|t| {
        t.parallel();

        let source = r#"
const x = 42 satisfies number
const y = "hello" satisfies string
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find at least 2 satisfies expression sites plus variable declarations
        let mut satisfies_count = 0_i64;
        for r in results.as_deref().unwrap_or_default() {
            if r.expected_type(&mut tp).is_some() && r.real_type(&mut tp).is_some() {
                satisfies_count += 1;
            }
        }
        if satisfies_count < 2 {
            t.error(sprintf!(
                "expected at least 2 assignment sites (satisfies expressions), got %d valid results out of %d total",
                satisfies_count,
                results_len(results.as_deref())
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:276 TestExpectedAndRealTypes_NilInputs 12e71b6083e406c1
// Rust: a nil `*TypeParser` cannot be written; Go's call also passes a nil source file, which the same guard
// rejects, so that input is checked on a parser.
#[test]
fn test_expected_and_real_types_nil_inputs() {
    run(|t| {
        t.parallel();

        let (mut compiled, _) = compile_and_get_checker_and_source_file(t, "");
        let mut tp = compiled.type_parser();

        // Test nil checker
        let results = tp.expected_and_real_types(None);
        if results.is_some() {
            t.error(sprintf!(
                "expected nil for nil checker, got %d results",
                results_len(results.as_deref())
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:286 TestEffectType_NilInputs 090d804c8bf08c82
// Rust: the nil checker case (a nil `*TypeParser` receiver) cannot be written and is left out.
#[test]
fn test_effect_type_nil_inputs() {
    run(|t| {
        t.parallel();

        let source = "const x: number = 42";
        let (mut compiled, _) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        // nil type must not panic
        if tp.effect_type(None).is_some() {
            t.error("expected nil for nil type");
        }
        // HasEffectTypeId with nil type must not panic
        if tp.has_effect_type_id(None) {
            t.error("expected false for nil type");
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:307 TestLayerType_NilInputs 40c69b6c6363acc2
// Rust: the nil checker case (a nil `*TypeParser` receiver) cannot be written and is left out.
#[test]
fn test_layer_type_nil_inputs() {
    run(|t| {
        t.parallel();

        let source = "const x: number = 42";
        let (mut compiled, _) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        if tp.layer_type(None).is_some() {
            t.error("expected nil for nil type");
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:322 TestContextTag_NilInputs 2951f63efa6cf832
// Rust: the nil checker case (a nil `*TypeParser` receiver) cannot be written and is left out.
#[test]
fn test_context_tag_nil_inputs() {
    run(|t| {
        t.parallel();

        let source = "const x: number = 42";
        let (mut compiled, _) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        if tp.context_tag(None).is_some() {
            t.error("expected nil for nil type");
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:337 TestExpectedAndRealTypes_EmptyFile 1bf81203ccec5b4a
#[test]
fn test_expected_and_real_types_empty_file() {
    run(|t| {
        t.parallel();

        let source = "";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        if results_len(results.as_deref()) != 0 {
            t.error(sprintf!(
                "expected 0 results for empty file, got %d",
                results_len(results.as_deref())
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:352 TestExpectedAndRealTypes_AllPatterns 6a4af26602a63fe6
#[test]
fn test_expected_and_real_types_all_patterns() {
    run(|t| {
        t.parallel();

        // A comprehensive source that exercises all 8 patterns in a single file
        let source = r#"
// Pattern 1: Variable declaration with initializer
const a: number = 42

// Pattern 2: Call expression arguments
function takesNum(n: number): void {}
takesNum(10)

// Pattern 3: Object literal property
interface Config { port: number; host: string }
const config: Config = { port: 3000, host: "localhost" }

// Pattern 4: Binary assignment
let x: number = 0
x = 99

// Pattern 5: Return statement
function getVal(): string {
  return "result"
}

// Pattern 6: Arrow function body (no type params)
const arrowFn: () => number = () => 123

// Pattern 7: Arrow function body (with type params)
function identity<T>(fn: () => T): T { return fn() }
const id = identity(<A extends string>(): A => "hi" as any as A)

// Pattern 8: Satisfies expression
const s = "test" satisfies string
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Should find multiple assignment sites
        if results_len(results.as_deref()) < 8 {
            t.error(sprintf!(
                "expected at least 8 assignment sites from all patterns combined, got %d",
                results_len(results.as_deref())
            ));
        }

        // Verify no nil types in results
        for (i, r) in results.as_deref().unwrap_or_default().iter().enumerate() {
            if r.expected_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: ExpectedType is nil", i as i64));
            }
            if r.real_type(&mut tp).is_none() {
                t.error(sprintf!("result[%d]: RealType is nil", i as i64));
            }
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:415 TestExpectedAndRealTypes_TypeStringVerification 212de00ff487fe81
#[test]
fn test_expected_and_real_types_type_string_verification() {
    run(|t| {
        t.parallel();

        let source = r#"
const a: number = 42
const b: string = "hello"
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(sf);

        // Verify we can use TypeToString on the results (confirms types are valid checker types)
        for (i, r) in results.as_deref().unwrap_or_default().iter().enumerate() {
            if let Some(expected_type) = r.expected_type(&mut tp) {
                let str = type_string(&mut tp, expected_type);
                if str.is_empty() {
                    t.error(sprintf!(
                        "result[%d]: TypeToString returned empty for ExpectedType",
                        i as i64
                    ));
                }
            }
            if let Some(real_type) = r.real_type(&mut tp) {
                let str = type_string(&mut tp, real_type);
                if str.is_empty() {
                    t.error(sprintf!(
                        "result[%d]: TypeToString returned empty for RealType",
                        i as i64
                    ));
                }
            }
        }
    });
}
