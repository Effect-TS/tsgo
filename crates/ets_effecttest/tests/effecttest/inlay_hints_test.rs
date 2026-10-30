// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::core::Tristate;
use ets_shim::ls::lsutil::{InlayHintsPreferences, UserPreferences};
use libtest_mimic::Trial;

use crate::hover_test::plugin_test_content;
use crate::inlay_hints_baseline_test::verify_local_baseline_inlay_hints;
use crate::run_case;

/// Rust-only: the shim's `lsutil.NewInlayHintUserPreferences(preferences)`
/// (`&lsutil.UserPreferences{InlayHintsPreferences: preferences}`).
fn new_inlay_hint_user_preferences(preferences: InlayHintsPreferences) -> UserPreferences {
    UserPreferences { inlay_hints: preferences, ..Default::default() }
}

/// Rust-only: Go's anonymous table struct of `TestEffectInlayHints`.
struct InlayHintsCase {
    name: &'static str,
    plugin_options: &'static str,
    source: &'static str,
}

// go test: internal/effecttest/inlay_hints_test.go:14 TestEffectInlayHints 42f105cb90d346e6
fn test_effect_inlay_hints(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    for tc in [
        InlayHintsCase {
            name: "effectInlayHintsGenSuppression",
            plugin_options: r#",
        "inlays": true"#,
            source: r#"import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sample = Effect.gen(function*() {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#,
        },
        InlayHintsCase {
            name: "effectInlayHintsFnSuppression",
            plugin_options: r#",
        "inlays": true"#,
            source: r#"import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sampleFn = Effect.fn("sampleFn")(function*(
  _arg1: number,
  _arg2: string
) {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#,
        },
        InlayHintsCase {
            name: "effectInlayHintsFnUntracedSuppression",
            plugin_options: r#",
        "inlays": true"#,
            source: r#"import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sampleFnUntraced = Effect.fnUntraced(function*(_: boolean) {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#,
        },
        InlayHintsCase {
            name: "effectInlayHintsDisabledPassthrough",
            plugin_options: "",
            source: r#"import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sample = Effect.gen(function*() {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#,
        },
    ] {
        t.run(tc.name, |t| {
            t.parallel();

            let content = plugin_test_content(tc.plugin_options, tc.source);
            let mut f = ets_fourslash::new_fourslash(t, None, &content);

            f.go_to_file(t, "/test.ts");
            verify_local_baseline_inlay_hints(
                t,
                &mut f,
                &content,
                "/test.ts",
                Some(new_inlay_hint_user_preferences(InlayHintsPreferences {
                    include_inlay_function_like_return_type_hints: Tristate::True,
                    ..Default::default()
                })),
            );
            f.done(t);
        });
    }
}

/// Rust-only: the trial of this file's Go test, named like it (the baseline file names come from its subtests).
pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case("TestEffectInlayHints".to_string(), test_effect_inlay_hints)]
}
