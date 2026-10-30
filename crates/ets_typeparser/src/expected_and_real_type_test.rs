// Ported from effect-tsgo's internal/typeparser/expected_and_real_type_test.go (package typeparser_test; see
// helpers_test.rs). Rust: `ExpectedAndRealType`'s `Node()` and `ValueNode()` are never nil here (`P<Node>`), so
// Go's nil checks on them hold trivially and are left out; a nil `*TypeParser` receiver cannot be written either,
// so the nil-parser half of TestTypeParser_NilInputs is left out (see the test).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::SourceFile;
use ets_shim::core::P;
use ets_shim::vfs::vfstest::MapFile;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/expected_and_real_type_test.go:23 compileAndGetCheckerAndSourceFile 27341b85ff88f55e
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

// go test: internal/typeparser/expected_and_real_type_test.go:63 TestExpectedAndRealTypes 136f1caf8509c28c
#[test]
fn test_expected_and_real_types() {
    run(|t| {
        t.parallel();

        let source = r#"
const a: number = 42

function takesNum(n: number): void {}
takesNum(10)

interface Config { port: number; host: string }
const config: Config = { port: 3000, host: "localhost" }

let x: number = 0
x = 99

function getVal(): string {
  return "result"
}

const arrowFn: () => number = () => 123

function identity<T>(fn: () => T): T { return fn() }
const id = identity(<A extends string>(): A => "hi" as any as A)

const s = "test" satisfies string
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file(t, source);
        let mut tp = compiled.type_parser();

        let mut got: Vec<String> = Vec::new();
        for r in tp.expected_and_real_types(sf).as_deref().unwrap_or_default() {
            let expected =
                r.expected_type(&mut tp).map(|ty| type_string(&mut tp, ty)).unwrap_or_default();
            let real = r.real_type(&mut tp).map(|ty| type_string(&mut tp, ty)).unwrap_or_default();
            got.push(format!(
                "Kind{:?} | {} | {} | {}",
                r.node().kind(),
                ets_shim::scanner::get_text_of_node(r.value_node()).trim(),
                expected,
                real
            ));
        }
        let want = [
            "KindNumericLiteral | 10 | number | 10",
            "KindIdentifier | 99 | number | 99",
            "KindIdentifier | 42 | number | 42",
            r#"KindIdentifier | { port: 3000, host: "localhost" } | Config | { port: number; host: string; }"#,
            "KindIdentifier | 0 | number | 0",
            r#"KindReturnStatement | return "result" | string | "result""#,
            "KindIdentifier | () => 123 | () => number | () => number",
            "KindReturnStatement | return fn() | T | T",
            r#"KindIdentifier | identity(<A extends string>(): A => "hi" as any as A) | string | string"#,
            r#"KindIdentifier | "test" satisfies string | "test" | "test""#,
            "KindNumericLiteral | 123 | number | 123",
            r#"KindArrowFunction | <A extends string>(): A => "hi" as any as A | () => string | <A extends string>() => A"#,
            r#"KindStringLiteral | "test" | string | "test""#,
            r#"KindAsExpression | "hi" as any as A | A | A"#,
            "KindIdentifier | port | number | number",
            "KindIdentifier | host | string | string",
        ];
        if got != want {
            t.error(sprintf!(
                "ExpectedAndRealTypes =\n%s\nwant\n%s",
                got.join("\n"),
                want.join("\n")
            ));
        }
    });
}

// go test: internal/typeparser/expected_and_real_type_test.go:125 TestExpectedAndRealTypes_EmptyFile 8793775a5480aed8
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

// go test: internal/typeparser/expected_and_real_type_test.go:136 TestTypeParser_NilInputs bd01fa5449490fc9
// Rust: a nil `*TypeParser` cannot be written, so the loop runs on the parser only; Go's nil-checker call also
// passes a nil source file, which the same guard rejects, so that input is checked on the parser.
#[test]
fn test_type_parser_nil_inputs() {
    run(|t| {
        t.parallel();

        let (mut compiled, _) = compile_and_get_checker_and_source_file(t, "const x: number = 42");
        let mut tp = compiled.type_parser();

        let results = tp.expected_and_real_types(None);
        if results.is_some() {
            t.error(sprintf!(
                "ExpectedAndRealTypes: expected nil for nil checker, got %d results",
                results_len(results.as_deref())
            ));
        }
        if tp.effect_type(None).is_some() {
            t.error("EffectType: expected nil for nil input");
        }
        if tp.layer_type(None).is_some() {
            t.error("LayerType: expected nil for nil input");
        }
        if tp.context_tag(None).is_some() {
            t.error("ContextTag: expected nil for nil input");
        }
        if tp.has_effect_type_id(None) {
            t.error("HasEffectTypeId: expected false for nil type");
        }
    });
}
