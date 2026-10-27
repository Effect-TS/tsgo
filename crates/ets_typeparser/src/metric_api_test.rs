// Ported from effect-tsgo's internal/typeparser/metric_api_test.go (see helpers_test.rs).

use ets_go::sprintf;

use crate::helpers_test::*;
use crate::returning_dispatch_test::find_returning_dispatch_test_function;
use crate::*;

// go test: internal/typeparser/metric_api_test.go:9 TestIsNodeReferenceToEffectMetricModuleApi fa717a7047709db1
#[test]
fn test_is_node_reference_to_effect_metric_module_api() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let source = r#"
				import { Effect, Metric } from "effect"
				import { update } from "effect/Metric"
				import * as M from "effect/Metric"
				const fake = { update: (_metric: unknown, _input: number) => Effect.void }
				const updateAlias = Metric.update
				const root = () => Metric.update
				const named = () => update
				const namespace = () => M.update
				const viaAlias = () => updateAlias
				const modify = () => Metric.modify
				const lookalike = () => fake.update
				const effectTap = () => Effect.tap
			"#;
                let cases: [(&str, &str, bool); 9] = [
                    ("root", "update", true),
                    ("named", "update", true),
                    ("namespace", "update", true),
                    ("viaAlias", "update", true),
                    ("modify", "update", false),
                    ("modify", "modify", true),
                    ("lookalike", "update", false),
                    ("effectTap", "update", false),
                    ("effectTap", "tap", false),
                ];
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, source,
                    );
                let mut tp = compiled.type_parser();
                for (name, member, want) in cases {
                    let function = find_returning_dispatch_test_function(t, sf, name);
                    let got = tp.is_node_reference_to_effect_metric_module_api(
                        get_function_like_body(function).unwrap(),
                        member,
                    );
                    if got != want {
                        t.error(sprintf!("%s as %s = %v, want %v", name, member, got, want));
                    }
                }
            });
        }
    });
}
