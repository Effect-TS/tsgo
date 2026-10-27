use ets_go::strconv::quote;
use ets_go::testing::on_checker_stack;

use crate::*;

// go test: internal/effecttest/match_when_stability_ts2589_test.go:13 TestIssue851MatchWhenStabilityDoesNotReportTS2589 3cce8426001aa7be
// Issue #851: the experimental and unstable API rules resolved the constraint
// of a generic call by resolving the call again with inference blocked. For a
// Match.when pattern over a wide union this instantiated
// WhenMatch<R, PatternBase<R>> on the shared checker and leaked TS2589 into the
// program's diagnostics.
#[test]
fn test_issue851_match_when_stability_does_not_report_ts2589() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            let source = r#"import { Match, Schema } from "effect"

const leaf = <K extends string, P extends string>(k: K, p: P) =>
  Schema.Struct({ k: Schema.Literal(k), p: Schema.Literal(p) })

const mid = <K extends string, P extends string>(k: K, p: P) =>
  Schema.Struct({ a: leaf(k, "#
                .to_string()
                + "`${p}a`"
                + "), b: Schema.Array(leaf(k, "
                + "`${p}b`"
                + ")), c: Schema.optional(leaf(k, "
                + "`${p}c`"
                + r#")) })

const variant = <K extends string>(k: K) =>
  Schema.Struct({
    kind: Schema.Literal(k),
    body: Schema.Struct({ a: mid(k, "a"), b: Schema.Array(mid(k, "b")), c: Schema.optional(mid(k, "c")) })
  })

const Node = Schema.Union([
  variant("A0"), variant("A1"), variant("A2"), variant("A3"), variant("A4"), variant("A5"), variant("A6"), variant("A7"),
  variant("B0"), variant("B1"), variant("B2"), variant("B3"), variant("B4"), variant("B5"), variant("B6"), variant("B7"),
  variant("C0"), variant("C1"), variant("C2"), variant("C3"), variant("C4"), variant("C5"), variant("C6"), variant("C7"),
  variant("D0"), variant("D1"), variant("D2"), variant("D3"), variant("D4"), variant("D5"), variant("D6"), variant("D7"),
  variant("E0"), variant("E1"), variant("E2"), variant("E3"), variant("E4"), variant("E5"), variant("E6"), variant("E7")
])

export const f = (node: typeof Node.Type) =>
  Match.value(node).pipe(
    Match.when({ kind: "A0" }, (v) => v.body),
    Match.orElse(() => null)
  )
"#;
            for severity in ["off", "error"] {
                t.run(&("rules=".to_string() + severity), |t| {
                    t.parallel();
                    let content = format!(
                        r#"// @filename: tsconfig.json
{{
  "compilerOptions": {{
    "strict": true,
    "plugins": [{{
      "name": "@effect/language-service",
      "diagnosticSeverity": {{
        "experimentalApiUsage": {},
        "unstableApiUsage": {}
      }}
    }}]
  }}
}}
// @filename: package.json
{{ "type": "module" }}
// @filename: test.ts
"#,
                        quote(severity),
                        quote(severity)
                    ) + &source;
                    let got = collect_diagnostic_strings_from_content(t, &content);
                    if !got.is_empty() {
                        t.fatal(format!("diagnostics = {got:?}, want none"));
                    }
                });
            }
        });
    });
}
