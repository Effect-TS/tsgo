// Ported from effect-tsgo's internal/typeparser/cause_type_test.go (see helpers_test.rs).

use ets_go::sprintf;

use crate::helpers_test::*;
use crate::returning_dispatch_test::find_returning_dispatch_test_function;
use crate::*;

/// Rust-only: Go's function-local `causeReferenceCase` type.
struct CauseReferenceCase {
    name: &'static str,
    member: &'static str,
    want: bool,
}

// go test: internal/typeparser/cause_type_test.go:9 TestIsNodeReferenceToEffectCauseModuleApi
#[test]
fn test_is_node_reference_to_effect_cause_module_api() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let mut source = r#"
				import { Cause, Exit } from "effect"
				import * as C from "effect/Cause"
				import { die as causeDie, fail as causeFail } from "effect/Cause"
				const Local = { Cause: { fail: <E>(e: E) => e, die: (d: unknown) => d } }
				const failAlias = Cause.fail
				const namespace = () => Cause.fail
				const subpath = () => C.die
				const named = () => causeFail
				const namedDie = () => causeDie
				const alias = () => failAlias
				const empty = () => Cause.empty
				const lookalike = () => Local.Cause.fail
				const otherModule = () => Exit.fail
			"#
                .to_string();
                let case = |name, member, want| CauseReferenceCase { name, member, want };
                let mut cases = vec![
                    case("namespace", "fail", true),
                    case("namespace", "die", false),
                    case("subpath", "die", true),
                    case("named", "fail", true),
                    case("namedDie", "die", true),
                    case("alias", "fail", true),
                    case("empty", "empty", true),
                    case("lookalike", "fail", false),
                    case("otherModule", "fail", false),
                ];
                if version == EFFECT_V4 {
                    source += r"
				const noSuchElement = () => Cause.NoSuchElementError
			";
                    cases.push(case("noSuchElement", "NoSuchElementError", true));
                }
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, &source,
                    );
                let mut tp = compiled.type_parser();
                for tt in &cases {
                    let function = find_returning_dispatch_test_function(t, sf, tt.name);
                    let got = tp.is_node_reference_to_effect_cause_module_api(
                        get_function_like_body(function).unwrap(),
                        tt.member,
                    );
                    if got != tt.want {
                        t.error(sprintf!(
                            "%s as %s = %v, want %v",
                            tt.name,
                            tt.member,
                            got,
                            tt.want
                        ));
                    }
                }
            });
        }
    });
}

/// Rust-only: Go's anonymous case struct of `TestEffectModuleIdentifierAt`.
struct EffectModuleIdentifierAtCase {
    name: &'static str,
    source: &'static str,
    want: &'static str,
}

// go test: internal/typeparser/cause_type_test.go:63 TestEffectModuleIdentifierAt
#[test]
fn test_effect_module_identifier_at() {
    run(|t| {
        t.parallel();
        let cases = [
            EffectModuleIdentifierAtCase {
                name: "barrel",
                source: r#"import { Effect } from "effect"
const target = () => 1"#,
                want: "Effect",
            },
            EffectModuleIdentifierAtCase {
                name: "aliasedBarrel",
                source: r#"import { Effect as Fx } from "effect"
const target = () => 1"#,
                want: "Fx",
            },
            EffectModuleIdentifierAtCase {
                name: "namespace",
                source: r#"import * as Fx from "effect/Effect"
const target = () => 1"#,
                want: "Fx",
            },
            EffectModuleIdentifierAtCase {
                name: "namedImportsOnly",
                source: r#"import { fail } from "effect/Effect"
const target = () => fail"#,
                want: "",
            },
            EffectModuleIdentifierAtCase {
                name: "shadowed",
                source: r#"import { Effect } from "effect"
const outer = (Effect: string) => {
  const target = () => Effect
  return target
}"#,
                want: "",
            },
            EffectModuleIdentifierAtCase {
                name: "unrelatedEffect",
                source: r"const Effect = { fail: (e: unknown) => e }
const target = () => Effect",
                want: "",
            },
            EffectModuleIdentifierAtCase {
                name: "typeOnlyImport",
                source: r#"import type { Effect } from "effect"
const target = () => 1"#,
                want: "",
            },
            EffectModuleIdentifierAtCase {
                name: "typeOnlySpecifier",
                source: r#"import { type Effect } from "effect"
const target = () => 1"#,
                want: "",
            },
            EffectModuleIdentifierAtCase {
                name: "typeOnlyNamespace",
                source: r#"import type * as Fx from "effect/Effect"
const target = () => 1"#,
                want: "",
            },
        ];
        for tt in &cases {
            t.run(tt.name, |t| {
                t.parallel();
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, EFFECT_V4, tt.source,
                    );
                let mut tp = compiled.type_parser();
                let body =
                    get_function_like_body(find_returning_dispatch_test_function(t, sf, "target"));
                let got = tp.effect_module_identifier_at(body);
                if got != tt.want {
                    t.fatal(sprintf!("EffectModuleIdentifierAt = %q, want %q", got, tt.want));
                }
            });
        }
    });
}
