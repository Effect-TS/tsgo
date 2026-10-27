use ets_go::testing::on_checker_stack;

use crate::*;

// go test: internal/effecttest/never_spread_ts2488_test.go:14 TestIssue916NeverSpreadMatchesStockDiagnostics
// Issue #916: promiseInEffectSuccess and other rules queried the type of a
// spread element of a never-typed value, a node the checker only checks
// through its enclosing array literal or call, leaking TS2488 into the
// program's diagnostics. Stock checking accepts the array spread and rejects
// the call spread, and the plugin must report exactly the same.
#[test]
fn test_issue916_never_spread_matches_stock_diagnostics() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            let source = r#"declare function g(...values: Array<number>): void

export function f() {
  const rows = [1] as never;
  g(...rows);
  return [...rows];
}
"#;
            // Rust: Go ranges over a map; the subtests run in parallel, so the order does not matter.
            for (name, plugin) in [
                ("no plugin", ""),
                ("diagnostics off", r#"{ "name": "@effect/language-service", "diagnostics": false }"#),
                (
                    "rule off",
                    r#"{ "name": "@effect/language-service", "diagnosticSeverity": { "promiseInEffectSuccess": "off" } }"#,
                ),
                (
                    "rule error",
                    r#"{ "name": "@effect/language-service", "diagnosticSeverity": { "promiseInEffectSuccess": "error" } }"#,
                ),
            ] {
                t.run(name, |t| {
                    t.parallel();
                    let content = format!(
                        r#"// @filename: tsconfig.json
{{ "compilerOptions": {{ "strict": true, "plugins": [{}] }} }}
// @filename: package.json
{{ "type": "module" }}
// @filename: test.ts
"#,
                        plugin
                    ) + source;
                    let got = collect_diagnostic_strings_from_content(t, &content);
                    let want = vec![
                        "TS2488: Type 'never' must have a '[Symbol.iterator]()' method that returns an iterator."
                            .to_string(),
                    ];
                    if got != want {
                        t.fatal(format!("diagnostics = {got:?}, want {want:?}"));
                    }
                });
            }
        });
    });
}
