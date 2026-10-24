use ets_go::sprintf;

use crate::helpers_test::*;
use crate::identity_forwarder_test::find_variable_initializer;
use crate::*;

// go test: internal/typeparser/lazy_expression_test.go:5 TestParseLazyExpressionMultiStatementBlock
#[test]
fn test_parse_lazy_expression_multi_statement_block() {
    run(|t| {
        t.parallel();
        let (_compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"
const statements = () => { const value = 1; console.log(value); return value }
const nested = () => { const inner = () => { return 2 }; return inner() }
const early = () => { if (Math.random()) return 1; return 2 }
const bare = () => { if (Math.random()) return; return 2 }
const finallyReturn = () => { try { return 1 } finally {} return 2 }
const notFinal = () => { return 1; console.log(2) }
const empty = () => {}
const single = () => { return 3 }
",
        );
        for name in ["statements", "nested", "single"] {
            let Some(parsed) = parse_lazy_expression(
                find_variable_initializer(t, sf, name),
                LazyExpressionFlags::AllowMultiStatementBlock,
            ) else {
                t.fatal(sprintf!("%s: expected the final return expression", name));
            };
            let expression = parsed.expression();
            let pos = expression.pos().source_offset().expect("a parsed node") as usize;
            let end = expression.end().source_offset().expect("a parsed node") as usize;
            let text = &sf.text()[pos..end];
            let want = match name {
                "statements" => " value",
                "nested" => " inner()",
                "single" => " 3",
                _ => "",
            };
            if text != want {
                t.fatal(sprintf!("%s: expression = %q, want %q", name, text, want));
            }
        }
        for name in ["early", "bare", "finallyReturn", "notFinal", "empty"] {
            if parse_lazy_expression(
                find_variable_initializer(t, sf, name),
                LazyExpressionFlags::AllowMultiStatementBlock,
            )
            .is_some()
            {
                t.fatal(sprintf!("%s: expected rejection", name));
            }
        }
        if parse_lazy_expression(
            find_variable_initializer(t, sf, "statements"),
            LazyExpressionFlags::None,
        )
        .is_some()
        {
            t.fatal("the default parser must still reject multiple statements");
        }
    });
}

// go test: internal/typeparser/lazy_expression_test.go:39 TestParseLazyExpressionFlags
#[test]
fn test_parse_lazy_expression_flags() {
    run(|t| {
        t.parallel();

        let (_compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"
const sync = () => 1
const parenthesized = (((() => 1)))
const asyncFunction = async () => 1
const generator = function* () { return 1 }
const parameter = (value: number) => value
",
        );

        if parse_lazy_expression(
            find_variable_initializer(t, sf, "sync"),
            LazyExpressionFlags::None,
        )
        .is_none()
        {
            t.fatal("expected zero flags to accept a synchronous function");
        }
        if parse_lazy_expression(
            find_variable_initializer(t, sf, "parenthesized"),
            LazyExpressionFlags::None,
        )
        .is_none()
        {
            t.fatal("expected the parser to skip parentheses");
        }
        let async_function = find_variable_initializer(t, sf, "asyncFunction");
        if parse_lazy_expression(async_function, LazyExpressionFlags::None).is_some() {
            t.fatal("expected zero flags to reject an async function");
        }
        if parse_lazy_expression(async_function, LazyExpressionFlags::AllowAsync).is_none() {
            t.fatal("expected AllowAsync to accept an async function");
        }
        let generator = find_variable_initializer(t, sf, "generator");
        if parse_lazy_expression(generator, LazyExpressionFlags::None).is_some() {
            t.fatal("expected zero flags to reject a generator function");
        }
        if parse_lazy_expression(generator, LazyExpressionFlags::AllowGenerator).is_none() {
            t.fatal("expected AllowGenerator to accept a generator function");
        }
        if parse_lazy_expression(
            find_variable_initializer(t, sf, "parameter"),
            LazyExpressionFlags::Thunk,
        )
        .is_some()
        {
            t.fatal("expected Thunk to reject a function with parameters");
        }
    });
}
