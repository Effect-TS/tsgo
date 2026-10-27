// Port of internal/rules/inferred_generator_contextual_type_test.go.

use ets_go::testing::run;
use ets_rule::Rule;
use ets_shim::core::context::Context as GoContext;
use ets_shim::diagnostics::Category;

use crate::api_stability_leak_test::compile_api_stability_source_primed;
use crate::*;

// go test: internal/rules/inferred_generator_contextual_type_test.go:14 TestInferredGeneratorStabilityPreservesNativeErrors 665fbfb73d1f3571
#[test]
fn test_inferred_generator_stability_preserves_native_errors() {
    run(|t| {
        t.parallel();

        const SOURCE: &str = r"interface Options {
  /** @stability experimental */
  experimental?: number;
  /** @stability unstable */
  unstable?: number;
}
declare function accept(options: Options): void;
declare function constrained<T extends Options>(options: T): T;
constrained({ experimental: 1, unstable: 2 });
const inferred = function* () {
  accept({ experimental: 3, unstable: 4 });
  constrained({ experimental: 5, unstable: 6 });
  yield* 1;
  const invalid = { ...1 };
};
const annotated = function* (): Generator<never, Options, unknown> {
  return constrained({ experimental: 7, unstable: 8 });
};
";
        for prime in [false, true] {
            for experimental_first in [false, true] {
                t.run(&format!("prime={prime}/experimentalFirst={experimental_first}"), |t| {
                    t.parallel();
                    let mut compiled = compile_api_stability_source_primed(t, SOURCE, prime);
                    let sf = compiled.source_file;
                    compiled.with_rule_ctx(|ctx| {
                        let mut tests: Vec<(&Rule, [&str; 4])> = vec![
                            (
                                &EXPERIMENTAL_API_USAGE,
                                [
                                    "experimental: 1",
                                    "experimental: 3",
                                    "experimental: 5",
                                    "experimental: 7",
                                ],
                            ),
                            (
                                &UNSTABLE_API_USAGE,
                                ["unstable: 2", "unstable: 4", "unstable: 6", "unstable: 8"],
                            ),
                        ];
                        if !experimental_first {
                            tests.reverse();
                        }
                        for (rule, uses) in &tests {
                            let diagnostics = (rule.run)(ctx);
                            if diagnostics.len() != uses.len() {
                                let texts: Vec<String> =
                                    diagnostics.iter().map(|d| d.localize()).collect();
                                t.fatal(format!(
                                    "{}: diagnostics = [{}], want the outside, normal-context, constraint, and annotated-generator warnings",
                                    rule.name,
                                    texts.join(" ")
                                ));
                            }
                            for (i, diagnostic) in diagnostics.iter().enumerate() {
                                let name = uses[i].split(':').next().unwrap_or_default();
                                let pos = diagnostic.pos().unwrap_or_default() as usize;
                                let len = diagnostic.len().unwrap_or_default() as usize;
                                if Some(pos) != SOURCE.find(uses[i])
                                    || SOURCE.get(pos..pos + len) != Some(name)
                                {
                                    t.error(ets_go::sprintf!(
                                        "%s: diagnostic %d has range %d:%d, want %q",
                                        rule.name,
                                        i,
                                        pos,
                                        len,
                                        uses[i]
                                    ));
                                }
                                if !diagnostic.localize().contains(&format!("effect({})", rule.name))
                                {
                                    t.error(format!(
                                        "unexpected stability diagnostic: {}",
                                        diagnostic.localize()
                                    ));
                                }
                            }
                        }
                        let diagnostics = ets_shim::compiler::sort_and_deduplicate_diagnostics(
                            &ctx.checker().get_diagnostics_exported(&GoContext::background(), sf),
                        );
                        let mut got = Vec::new();
                        for diagnostic in &diagnostics {
                            if diagnostic.category() == Category::Error {
                                got.push(format!(
                                    "{}:{}:{}",
                                    diagnostic.code(),
                                    diagnostic.pos().unwrap_or_default(),
                                    diagnostic.len().unwrap_or_default()
                                ));
                            }
                        }
                        got.sort();
                        let want = vec![
                            format!(
                                "2488:{}:1",
                                SOURCE.find("yield* 1").expect("in the source") + "yield* ".len()
                            ),
                            format!("2698:{}:4", SOURCE.find("...1").expect("in the source")),
                        ];
                        if got != want {
                            t.fatal(format!(
                                "native diagnostic ranges = [{}], want [{}]",
                                got.join(" "),
                                want.join(" ")
                            ));
                        }
                    });
                });
            }
        }
    });
}
