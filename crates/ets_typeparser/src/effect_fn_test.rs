use std::sync::Arc;

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/effect_fn_test.go:10 findFirstEffectFnCall
fn find_first_effect_fn_call(
    t: &T,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Arc<EffectFnCallResult> {
    t.helper();
    fn visit(tp: &mut TypeParser<'_>, node: P<Node>, result: &mut Option<Arc<EffectFnCallResult>>) {
        if result.is_some() {
            return;
        }
        if let Some(parsed) = tp.effect_fn_call(node) {
            *result = Some(parsed);
            return;
        }
        node.for_each_child(&mut |child| {
            visit(tp, child, result);
            false
        });
    }
    let mut result = None;
    visit(tp, sf.as_node(), &mut result);
    let Some(result) = result else {
        t.fatal("expected to find Effect.fn call");
    };
    result
}

// go test: internal/typeparser/effect_fn_test.go:34 compileEffectFnTest
// Rust: the parser borrows the returned `Compiled` (`compiled.type_parser()`); dropping it is Go's `done`.
fn compile_effect_fn_test(
    t: &T,
    version: EffectVersion,
    source: &str,
) -> (Compiled, P<SourceFile>) {
    t.helper();
    compile_and_get_checker_and_source_file_with_effect_version_internal(t, version, source)
}

// go test: internal/typeparser/effect_fn_test.go:40 assertEffectFnFunctionReturnType
// Rust: Go's `parsed.functionReturnType == nil` checks the lazy itself, which Rust's field cannot be; a nil
// resolved type (which Go would hand to TypeToString) fails with the same message.
fn assert_effect_fn_function_return_type(t: &T, version: EffectVersion, source: &str, want: &str) {
    t.helper();
    let (mut compiled, sf) =
        compile_and_get_checker_and_source_file_with_effect_version_internal(t, version, source);
    let mut tp = compiled.type_parser();

    let parsed = find_first_effect_fn_call(t, &mut tp, sf);
    let Some(function_return_type) = parsed.function_return_type(&mut tp) else {
        t.fatal("expected FunctionReturnType");
    };
    let got = type_string(&mut tp, function_return_type);
    if got != want {
        t.fatal(sprintf!("FunctionReturnType = %q", got));
    }
}

// go test: internal/typeparser/effect_fn_test.go:54 TestEffectFnCall_ConstantAlias
#[test]
fn test_effect_fn_call_constant_alias() {
    run(|t| {
        t.parallel();

        for version in [EFFECT_V3, EFFECT_V4] {
            let (mut compiled, sf) = compile_effect_fn_test(
                t,
                version,
                r#"
import { Effect } from "effect"

const fn = Effect.fn
export const make = fn(function*() {
  return 1
})
"#,
            );
            let mut tp = compiled.type_parser();
            let parsed = find_first_effect_fn_call(t, &mut tp, sf);
            if parsed.effect_module.is_some() {
                t.fatal(
                    "expected a constant fn alias to have no directly replaceable Effect module receiver",
                );
            }
        }
    });
}

// go test: internal/typeparser/effect_fn_test.go:75 TestEffectFnCall_FunctionReturnType_NoPipe
#[test]
fn test_effect_fn_call_function_return_type_no_pipe() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.fn(function*(n: number) {
  return n + 1
})
"#;
        assert_effect_fn_function_return_type(t, EFFECT_V4, source, "Effect<number, never, never>");
        assert_effect_fn_function_return_type(t, EFFECT_V3, source, "Effect<number, never, never>");
    });
}

// go test: internal/typeparser/effect_fn_test.go:89 TestEffectFnCall_FunctionReturnType_FirstPipeInput
#[test]
fn test_effect_fn_call_function_return_type_first_pipe_input() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.fn(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
"#;
        assert_effect_fn_function_return_type(t, EFFECT_V4, source, "Effect<number, never, never>");
        assert_effect_fn_function_return_type(t, EFFECT_V3, source, "Effect<number, never, never>");
    });
}

// go test: internal/typeparser/effect_fn_test.go:106 TestEffectFnCall_FunctionReturnType_Generic
#[test]
fn test_effect_fn_call_function_return_type_generic() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A>(a: A) {
    return a
  },
  Effect.map((a) => [a] as const)
)
"#;
        assert_effect_fn_function_return_type(t, EFFECT_V4, source, "Effect<A, never, never>");
        assert_effect_fn_function_return_type(t, EFFECT_V3, source, "Effect<A, never, never>");
    });
}

// go test: internal/typeparser/effect_fn_test.go:123 TestEffectFnCall_FunctionReturnType_GenericNoPipe
#[test]
fn test_effect_fn_call_function_return_type_generic_no_pipe() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.fn(function*<A>(a: A) {
  return a
})
"#;
        assert_effect_fn_function_return_type(t, EFFECT_V4, source, "Effect<A, never, never>");
        assert_effect_fn_function_return_type(t, EFFECT_V3, source, "Effect<A, never, never>");
    });
}

// go test: internal/typeparser/effect_fn_test.go:137 TestEffectFnCall_FunctionReturnType_ConstrainedGeneric
#[test]
fn test_effect_fn_call_function_return_type_constrained_generic() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A extends { readonly id: string }>(a: A) {
    return a
  },
  Effect.tap((a) => Effect.log(a.id))
)
"#;
        assert_effect_fn_function_return_type(t, EFFECT_V4, source, "Effect<A, never, never>");
        assert_effect_fn_function_return_type(t, EFFECT_V3, source, "Effect<A, never, never>");
    });
}

// go test: internal/typeparser/effect_fn_test.go:154 TestEffectFnCall_FunctionReturnType_NamedSpan
#[test]
fn test_effect_fn_call_function_return_type_named_span() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.fn(
  "named-span"
)(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
"#;
        for version in [EFFECT_V4, EFFECT_V3] {
            let (mut compiled, sf) =
                compile_and_get_checker_and_source_file_with_effect_version_internal(
                    t, version, source,
                );
            let mut tp = compiled.type_parser();
            let parsed = find_first_effect_fn_call(t, &mut tp, sf);
            if parsed.trace_expression.is_none() {
                t.fatal("expected TraceExpression");
            }
            // Rust: see assert_effect_fn_function_return_type for the lazy's nil check.
            let Some(function_return_type) = parsed.function_return_type(&mut tp) else {
                t.fatal("expected FunctionReturnType");
            };
            let got = type_string(&mut tp, function_return_type);
            if got != "Effect<number, never, never>" {
                t.fatal(sprintf!("FunctionReturnType = %q", got));
            }
        }
    });
}

// go test: internal/typeparser/effect_fn_test.go:188 TestEffectFnCall_FunctionReturnType_SelfObject
#[test]
fn test_effect_fn_call_function_return_type_self_object() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v4_internal(
            t,
            r#"
import { Effect } from "effect"

const self = {
  prefix: "value"
}

export const make = Effect.fn(
  { self },
  function*(this: typeof self, n: number) {
    return this.prefix + n.toString()
  },
  Effect.map((value) => value.length)
)
"#,
        );
        let mut tp = compiled.type_parser();

        let parsed = find_first_effect_fn_call(t, &mut tp, sf);
        if parsed.options_node.is_none() {
            t.fatal("expected OptionsNode");
        }
        // Rust: see assert_effect_fn_function_return_type for the lazy's nil check.
        let Some(function_return_type) = parsed.function_return_type(&mut tp) else {
            t.fatal("expected FunctionReturnType");
        };
        let got = type_string(&mut tp, function_return_type);
        if got != "Effect<string, never, never>" {
            t.fatal(sprintf!("FunctionReturnType = %q", got));
        }
    });
}
