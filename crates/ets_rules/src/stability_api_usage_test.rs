// Ported from effect-tsgo's internal/rules/stability_api_usage_test.go (BenchmarkStabilityApiUsageContextualProperties
// is listed in porting/skipped.tsv).

use ets_go::testing::T;
use ets_rule::Rule;
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFileParseOptions};
use ets_shim::core::ScriptKind;

use crate::api_stability_leak_test::*;
use crate::*;

/// Rust-only: runs a ported test on a checker-sized stack.
fn run(f: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || ets_go::testing::run(f));
}

/// Rust-only: Go's `diagnostic.Pos()` (an `int`).
fn diagnostic_pos(diagnostic: P<Diagnostic>) -> usize {
    diagnostic.pos().expect("a diagnostic with a location") as usize
}

/// Rust-only: Go's `source[diagnostic.Pos() : diagnostic.Pos()+diagnostic.Len()]`.
fn diagnostic_text(source: &str, diagnostic: P<Diagnostic>) -> &str {
    let pos = diagnostic_pos(diagnostic);
    let len = diagnostic.len().expect("a diagnostic with a location") as usize;
    &source[pos..pos + len]
}

// go test: internal/rules/stability_api_usage_test.go:17 TestStabilityOfDeclaration a79a007f5216824d
#[test]
fn test_stability_of_declaration() {
    run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str)] = &[
            ("unstable variable", "/** @stability unstable */\nexport const api = 1", "unstable"),
            (
                "experimental variable",
                "/** @stability experimental */\nexport const api = 1",
                "experimental",
            ),
            ("stable variable", "/** @stability stable */\nexport const api = 1", "stable"),
            ("other tag", "/** @deprecated */\nexport const api = 1", ""),
            ("plain variable", "export const api = 1", ""),
        ];
        for &(name, source, want) in tests {
            t.run(name, |t| {
                t.parallel();
                let sf = ets_shim::parser::parse_source_file(
                    SourceFileParseOptions {
                        file_name: "/test.ts".to_string(),
                        ..Default::default()
                    },
                    source,
                    ScriptKind::TS,
                );
                let statement = sf.statements().nodes()[0];
                let declaration = statement
                    .as_variable_statement()
                    .declaration_list()
                    .as_variable_declaration_list()
                    .declarations()
                    .nodes()[0];
                let got = ets_typeparser::stability_tag_of_declaration(declaration);
                if got != want {
                    t.error(ets_go::sprintf!("StabilityTagOfDeclaration = %q, want %q", got, want));
                }
            });
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:43 TestStabilityApiModuleName cf782c93864dd5ff
#[test]
fn test_stability_api_module_name() {
    run(|t| {
        t.parallel();
        for (file, want) in [
            ("/pkg/src/http/HttpClient.ts", "effect/http/HttpClient"),
            ("/pkg/dist/http/HttpClient.d.ts", "effect/http/HttpClient"),
            ("/pkg/dist/dts/http/HttpClient.d.mts", "effect/http/HttpClient"),
            ("/pkg/dist/cjs/http/HttpClient.d.cts", "effect/http/HttpClient"),
            ("/pkg/dist/http/index.d.ts", "effect/http"),
            ("/pkg/index.ts", "effect"),
            ("/pkg/types/client.d.ts", "effect/types/client"),
            ("/pkg-other/client.ts", ""),
        ] {
            t.run(file, |t| {
                t.parallel();
                let got = stability_api_module_name("effect", "/pkg", file);
                if got != want {
                    t.fatal(ets_go::sprintf!("got %q, want %q", got, want));
                }
            });
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:64 TestStabilityApiUsageContextualRuleOrder 05dfb065278841b5
#[test]
fn test_stability_api_usage_contextual_rule_order() {
    run(|t| {
        t.parallel();
        let repeated = |call: &str| -> String {
            format!(
                "interface Options {{
  /** @stability experimental */
  first: number
  stable?: number
  /** @stability unstable */
  second: number
  nested?: Options
}}

declare function accept(options: Options): void
declare function generic<T extends Options>(options: T): void
{call}({{ first: 1, stable: 2, second: 3, nested: {{ first: 4, second: 5 }} }})
const unrelated = {{ first: 8, second: 9 }}
accept({{ first: 6, second: 7 }})
"
            )
        };
        let repeated_experimental: &[&str] = &["first: 1", "first: 4", "first: 6"];
        let repeated_unstable: &[&str] = &["second: 3", "second: 5", "second: 7"];
        let discriminants = r#"interface A {
  kind: "a"
  /** @stability experimental */
  x: number
  y?: number
}
interface B {
  kind: "b"
  x: number
  /** @stability unstable */
  y?: number
}
declare function pick<T extends A | B>(value: T): T
pick({ kind: "a", x: pick({ kind: "b", x: 1, y: 2 }).x, y: 3 })
pick({ kind: "b", y: pick({ kind: "a", x: 4 }).x, x: 5 })
"#
        .to_string();
        let tests: [(&str, String, bool, &[&str], &[&str]); 4] = [
            ("repeated/accept", repeated("accept"), true, repeated_experimental, repeated_unstable),
            (
                "repeated/generic",
                repeated("generic"),
                true,
                repeated_experimental,
                repeated_unstable,
            ),
            (
                "discriminants/prime=true",
                discriminants.clone(),
                true,
                &["x: pick", "x: 4"],
                &["y: 2", "y: pick"],
            ),
            (
                "discriminants/prime=false",
                discriminants,
                false,
                &["x: pick", "x: 4"],
                &["y: 2", "y: pick"],
            ),
        ];
        for (name, source, prime, experimental, unstable) in &tests {
            let rules: [(&Rule, &[&str]); 2] =
                [(&EXPERIMENTAL_API_USAGE, experimental), (&UNSTABLE_API_USAGE, unstable)];
            for order in [[0, 1], [1, 0]] {
                t.run(&format!("{}/{}-first", name, rules[order[0]].0.name), |t| {
                    t.parallel();
                    let mut compiled = compile_api_stability_source_primed(t, source, *prime);
                    compiled.with_rule_ctx(|ctx| {
                        for index in order {
                            let (rule, uses) = &rules[index];
                            let mut got = Vec::new();
                            let mut want = Vec::new();
                            for diagnostic in (rule.run)(ctx) {
                                got.push(format!(
                                    "{}:{}",
                                    diagnostic_pos(diagnostic),
                                    diagnostic_text(source, diagnostic)
                                ));
                            }
                            for usage in *uses {
                                let (name, _) = usage.split_once(':').unwrap_or((usage, ""));
                                want.push(format!(
                                    "{}:{}",
                                    source.find(usage).expect("the use is in the source"),
                                    name
                                ));
                            }
                            if got != want {
                                t.error(ets_go::sprintf!(
                                    "%s: got %s, want %s",
                                    rule.name,
                                    format!("[{}]", got.join(" ")),
                                    format!("[{}]", want.join(" "))
                                ));
                            }
                        }
                    });
                });
            }
        }
    });
}
