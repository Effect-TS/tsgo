use std::sync::Arc;

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/effect_gen_test.go:10 findFirstEffectGenCall 3c8b0ceef2b1c002
fn find_first_effect_gen_call(
    t: &T,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Arc<EffectGenCallResult> {
    t.helper();
    fn visit(
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        result: &mut Option<Arc<EffectGenCallResult>>,
    ) {
        if result.is_some() {
            return;
        }
        if let Some(parsed) = tp.effect_gen_call(node) {
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
        t.fatal("expected to find Effect.gen call");
    };
    result
}

// go test: internal/typeparser/effect_gen_test.go:34 assertEffectGenFunctionReturnType bb1194f598d1c072
fn assert_effect_gen_function_return_type(t: &T, version: EffectVersion, source: &str, want: &str) {
    t.helper();
    let (mut compiled, sf) =
        compile_and_get_checker_and_source_file_with_effect_version_internal(t, version, source);
    let mut tp = compiled.type_parser();

    let parsed = find_first_effect_gen_call(t, &mut tp, sf);
    let Some(return_type) = tp.get_type_at_location(parsed.call) else {
        t.fatal("expected return type");
    };
    let got = type_string(&mut tp, return_type);
    if got != want {
        t.fatal(sprintf!("FunctionReturnType = %q", got));
    }
}

// go test: internal/typeparser/effect_gen_test.go:49 TestEffectGenCall_FunctionReturnType_NoPipe c45605b49d13ebc7
#[test]
fn test_effect_gen_call_function_return_type_no_pipe() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = Effect.gen(function*() {
  return 1
})
"#;
        assert_effect_gen_function_return_type(
            t,
            EFFECT_V4,
            source,
            "Effect<number, never, never>",
        );
        assert_effect_gen_function_return_type(
            t,
            EFFECT_V3,
            source,
            "Effect<number, never, never>",
        );
    });
}

// go test: internal/typeparser/effect_gen_test.go:63 TestEffectGenCall_FunctionReturnType_GenericNoPipe 9e24da6ad43785e8
#[test]
fn test_effect_gen_call_function_return_type_generic_no_pipe() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

export const make = <A>(a: A) => Effect.gen(function*() {
  return a
})
"#;
        assert_effect_gen_function_return_type(t, EFFECT_V4, source, "Effect<A, never, never>");
        assert_effect_gen_function_return_type(t, EFFECT_V3, source, "Effect<A, never, never>");
    });
}

// go test: internal/typeparser/effect_gen_test.go:77 TestEffectGenCall_FunctionReturnType_SelfObject 0c81e4ea898e9f23
#[test]
fn test_effect_gen_call_function_return_type_self_object() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

const self = {
  prefix: "value"
}

export const make = Effect.gen({ self }, function*(this: typeof self) {
  return this.prefix
})
"#;
        assert_effect_gen_function_return_type(
            t,
            EFFECT_V4,
            source,
            "Effect<string, never, never>",
        );
    });
}

// go test: internal/typeparser/effect_gen_test.go:94 TestEffectGenCall_FunctionReturnType_GenericSelfObject 123f61ec8ffcedd1
#[test]
fn test_effect_gen_call_function_return_type_generic_self_object() {
    run(|t| {
        t.parallel();

        let source_v4 = r#"
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen({ self }, function*(this: typeof self) {
  return this.value
})
"#;
        let source_v3 = r#"
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen(self, function*(this: typeof self) {
  return this.value
})
"#;
        assert_effect_gen_function_return_type(t, EFFECT_V4, source_v4, "Effect<A, never, never>");
        assert_effect_gen_function_return_type(t, EFFECT_V3, source_v3, "Effect<A, never, never>");
    });
}

// go test: internal/typeparser/effect_gen_test.go:115 TestEffectGenCall_ConstantAlias 222341ccc440b210
#[test]
fn test_effect_gen_call_constant_alias() {
    run(|t| {
        t.parallel();

        for version in [EFFECT_V3, EFFECT_V4] {
            let (mut compiled, sf) =
                compile_and_get_checker_and_source_file_with_effect_version_internal(
                    t,
                    version,
                    r#"
import { Effect } from "effect"

const gen = Effect.gen
export const make = gen(function*() {
  return 1
})
"#,
                );
            let mut tp = compiled.type_parser();
            let parsed = find_first_effect_gen_call(t, &mut tp, sf);
            if parsed.effect_module.is_some() {
                t.fatal(
                    "expected a constant gen alias to have no directly replaceable Effect module receiver",
                );
            }
        }
    });
}
