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
