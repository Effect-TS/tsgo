// Ported from effect-tsgo's internal/typeparser/function_module_test.go (see helpers_test.rs).

use ets_go::sprintf;

use crate::helpers_test::*;
use crate::returning_dispatch_test::find_returning_dispatch_test_function;
use crate::*;

// go test: internal/typeparser/function_module_test.go:9 functionReferenceCase 6da1b419a2788bff
struct FunctionReferenceCase {
    name: &'static str,
    member: &'static str,
    want: bool,
    via_local_variable: bool,
}

// go test: internal/typeparser/function_module_test.go:16 TestIsNodeReferenceToEffectFunctionModuleApi da0d6f3bb0850ff1
#[test]
fn test_is_node_reference_to_effect_function_module_api() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let mut source = r#"
				import { Channel, Function, identity } from "effect"
				import { identity as id } from "effect/Function"
				import * as F from "effect/Function"
				const localIdentity = <A>(a: A): A => a
				const fake = { identity: <A>(a: A): A => a }
				const root = () => identity
				const aliased = () => id
				const namespace = () => Function.identity
				const subpath = () => F.identity
				const otherExport = () => F.pipe
				const channel = () => Channel.identity
				const local = () => localIdentity
				const lookalike = () => fake.identity
				const plainAlias = identity
				const typedAlias: (x: string) => string = identity
				const viaAlias = () => plainAlias
				const viaTypedAlias = () => typedAlias
			"#
                .to_string();
                let case = |name, member, want, via_local_variable| FunctionReferenceCase {
                    name,
                    member,
                    want,
                    via_local_variable,
                };
                let mut cases = vec![
                    case("root", "identity", true, false),
                    case("aliased", "identity", true, false),
                    case("namespace", "identity", true, false),
                    case("subpath", "identity", true, false),
                    case("otherExport", "identity", false, false),
                    case("otherExport", "pipe", true, false),
                    case("channel", "identity", false, false),
                    case("local", "identity", false, false),
                    case("lookalike", "identity", false, false),
                    case("viaAlias", "identity", true, true),
                    case("viaTypedAlias", "identity", true, true),
                ];
                if version == EFFECT_V3 {
                    source += r#"
				import { Schedule, Stream } from "effect"
				const stream = () => Stream.identity
				const schedule = () => Schedule.identity
			"#;
                    cases.push(case("stream", "identity", false, false));
                    cases.push(case("schedule", "identity", false, false));
                }
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, &source,
                    );
                let mut tp = compiled.type_parser();
                for tt in &cases {
                    let function = find_returning_dispatch_test_function(t, sf, tt.name);
                    let body = get_function_like_body(function).unwrap();
                    let got = tp.is_node_reference_to_effect_function_module_api(body, tt.member);
                    if got != tt.want {
                        t.error(sprintf!(
                            "%s as %s = %v, want %v",
                            tt.name,
                            tt.member,
                            got,
                            tt.want
                        ));
                    }
                    let want_direct = tt.want && !tt.via_local_variable;
                    let got =
                        tp.is_node_direct_reference_to_effect_function_module_api(body, tt.member);
                    if got != want_direct {
                        t.error(sprintf!(
                            "%s as direct %s = %v, want %v",
                            tt.name,
                            tt.member,
                            got,
                            want_direct
                        ));
                    }
                }
            });
        }
    });
}
