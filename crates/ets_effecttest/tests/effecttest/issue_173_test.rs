// Rust: Go's blank imports of `etscheckerhooks`, `etslshooks` and `etstesthooks`: each test registers them
// (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use libtest_mimic::Trial;

use crate::run_case;

// go test: internal/effecttest/issue_173_test.go:13 TestIssue173_DirectYieldedThisDoesNotReportFalsePositiveTS2683 07425f8d7b282ca1
fn test_issue173_direct_yielded_this_does_not_report_false_positive_ts2683(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /repro.ts
import { Effect, Exit, Scope } from "effect"

class Repro {
  readonly #scope: Scope.Closeable = Scope.makeUnsafe()

  run(): Effect.Effect<void> {
    return Effect.gen({ self: this }, function* () {
      yield* Scope.close([|this|].#scope, Exit.void)
    })
  }
}"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.verify_non_suggestion_diagnostics(t, &[]);
    f.done(t);
}

// go test: internal/effecttest/issue_173_test.go:49 TestIssue173_HoistedThisScopeDoesNotReportTS2683 41cb6568fe2e0a21
fn test_issue173_hoisted_this_scope_does_not_report_ts2683(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /repro.ts
import { Effect, Exit, Scope } from "effect"

class Repro {
  readonly #scope: Scope.Closeable = Scope.makeUnsafe()

  run(): Effect.Effect<void> {
    return Effect.gen({ self: this }, function* () {
      const scope = this.#scope
      yield* Scope.close(scope, Exit.void)
    })
  }
}"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.verify_non_suggestion_diagnostics(t, &[]);
    f.done(t);
}

/// Rust-only: the Go test functions of `issue_173_test.go`, one trial each.
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case(
            "TestIssue173_DirectYieldedThisDoesNotReportFalsePositiveTS2683".to_string(),
            test_issue173_direct_yielded_this_does_not_report_false_positive_ts2683,
        ),
        run_case(
            "TestIssue173_HoistedThisScopeDoesNotReportTS2683".to_string(),
            test_issue173_hoisted_this_scope_does_not_report_ts2683,
        ),
    ]
}
