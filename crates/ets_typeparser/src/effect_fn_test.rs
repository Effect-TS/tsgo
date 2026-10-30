use std::sync::Arc;

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/effect_fn_test.go:10 findFirstEffectFnCall 5cc969ba06a09f41
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

// go test: internal/typeparser/effect_fn_test.go:34 TestEffectFnCall_ConstantAlias f1b7aba21c521a57
#[test]
fn test_effect_fn_call_constant_alias() {
    run(|t| {
        t.parallel();

        for version in [EFFECT_V3, EFFECT_V4] {
            let (mut compiled, sf) =
                compile_and_get_checker_and_source_file_with_effect_version_internal(
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

// go test: internal/typeparser/effect_fn_test.go:55 TestEffectFnCall_FunctionReturnType d4bef6d4b0ecb743
#[test]
fn test_effect_fn_call_function_return_type() {
    run(|t| {
        t.parallel();

        struct Case {
            name: &'static str,
            versions: &'static [EffectVersion],
            source: &'static str,
            want: &'static str,
            want_trace: bool,
            want_options: bool,
        }
        const BOTH_VERSIONS: &[EffectVersion] = &[EFFECT_V4, EFFECT_V3];
        let tests = [
            Case {
                name: "NoPipe",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.fn(function*(n: number) {
  return n + 1
})
"#,
                want: "Effect<number, never, never>",
                want_trace: false,
                want_options: false,
            },
            Case {
                name: "FirstPipeInput",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.fn(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
"#,
                want: "Effect<number, never, never>",
                want_trace: false,
                want_options: false,
            },
            Case {
                name: "Generic",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A>(a: A) {
    return a
  },
  Effect.map((a) => [a] as const)
)
"#,
                want: "Effect<A, never, never>",
                want_trace: false,
                want_options: false,
            },
            Case {
                name: "GenericNoPipe",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.fn(function*<A>(a: A) {
  return a
})
"#,
                want: "Effect<A, never, never>",
                want_trace: false,
                want_options: false,
            },
            Case {
                name: "ConstrainedGeneric",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A extends { readonly id: string }>(a: A) {
    return a
  },
  Effect.tap((a) => Effect.log(a.id))
)
"#,
                want: "Effect<A, never, never>",
                want_trace: false,
                want_options: false,
            },
            Case {
                name: "NamedSpan",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.fn(
  "named-span"
)(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
"#,
                want: "Effect<number, never, never>",
                want_trace: true,
                want_options: false,
            },
            Case {
                name: "SelfObject",
                versions: &[EFFECT_V4],
                source: r#"
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
                want: "Effect<string, never, never>",
                want_trace: false,
                want_options: true,
            },
        ];

        for tt in &tests {
            for &version in tt.versions {
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, tt.source,
                    );
                let mut tp = compiled.type_parser();
                let parsed = find_first_effect_fn_call(t, &mut tp, sf);
                let got = parsed.trace_expression.is_some();
                if got != tt.want_trace {
                    t.error(sprintf!(
                        "%s/%s: has trace expression = %v, want %v",
                        tt.name,
                        version,
                        got,
                        tt.want_trace
                    ));
                }
                let got = parsed.options_node.is_some();
                if got != tt.want_options {
                    t.error(sprintf!(
                        "%s/%s: has options node = %v, want %v",
                        tt.name,
                        version,
                        got,
                        tt.want_options
                    ));
                }
                // Rust: Go's `parsed.functionReturnType == nil` checks the lazy itself, which Rust's field cannot
                // be; a nil resolved type (which Go would hand to TypeToString) fails with the same message.
                match parsed.function_return_type(&mut tp) {
                    None => {
                        t.error(sprintf!("%s/%s: expected FunctionReturnType", tt.name, version));
                    }
                    Some(function_return_type) => {
                        let got = type_string(&mut tp, function_return_type);
                        if got != tt.want {
                            t.error(sprintf!(
                                "%s/%s: FunctionReturnType = %q, want %q",
                                tt.name,
                                version,
                                got,
                                tt.want
                            ));
                        }
                    }
                }
            }
        }
    });
}
