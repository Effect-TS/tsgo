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

// go test: internal/typeparser/effect_gen_test.go:34 TestEffectGenCall_FunctionReturnType 22babb382b0c2270
#[test]
fn test_effect_gen_call_function_return_type() {
    run(|t| {
        t.parallel();

        struct Case {
            name: &'static str,
            versions: &'static [EffectVersion],
            source: &'static str,
            want: &'static str,
        }
        const BOTH_VERSIONS: &[EffectVersion] = &[EFFECT_V4, EFFECT_V3];
        let tests = [
            Case {
                name: "NoPipe",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = Effect.gen(function*() {
  return 1
})
"#,
                want: "Effect<number, never, never>",
            },
            Case {
                name: "GenericNoPipe",
                versions: BOTH_VERSIONS,
                source: r#"
import { Effect } from "effect"

export const make = <A>(a: A) => Effect.gen(function*() {
  return a
})
"#,
                want: "Effect<A, never, never>",
            },
            Case {
                name: "SelfObject",
                versions: &[EFFECT_V4],
                source: r#"
import { Effect } from "effect"

const self = {
  prefix: "value"
}

export const make = Effect.gen({ self }, function*(this: typeof self) {
  return this.prefix
})
"#,
                want: "Effect<string, never, never>",
            },
            Case {
                name: "GenericSelfObject",
                versions: &[EFFECT_V4],
                source: r#"
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen({ self }, function*(this: typeof self) {
  return this.value
})
"#,
                want: "Effect<A, never, never>",
            },
            Case {
                name: "GenericSelfArgument",
                versions: &[EFFECT_V3],
                source: r#"
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen(self, function*(this: typeof self) {
  return this.value
})
"#,
                want: "Effect<A, never, never>",
            },
        ];

        for tt in &tests {
            for &version in tt.versions {
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, tt.source,
                    );
                let mut tp = compiled.type_parser();
                let parsed = find_first_effect_gen_call(t, &mut tp, sf);
                match tp.get_type_at_location(parsed.call) {
                    None => t.error(sprintf!("%s/%s: expected return type", tt.name, version)),
                    Some(return_type) => {
                        let got = type_string(&mut tp, return_type);
                        if got != tt.want {
                            t.error(sprintf!(
                                "%s/%s: return type = %q, want %q",
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

// go test: internal/typeparser/effect_gen_test.go:125 TestEffectGenCall_ConstantAlias 222341ccc440b210
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
