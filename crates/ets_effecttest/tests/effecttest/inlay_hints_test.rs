// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::core::Tristate;
use ets_shim::ls::lsutil::{InlayHintsPreferences, UserPreferences};
use libtest_mimic::Trial;

use crate::inlay_hints_baseline_test::verify_local_baseline_inlay_hints;
use crate::run_case;

/// Rust-only: the shim's `lsutil.NewInlayHintUserPreferences(preferences)`
/// (`&lsutil.UserPreferences{InlayHintsPreferences: preferences}`).
fn new_inlay_hint_user_preferences(preferences: InlayHintsPreferences) -> UserPreferences {
    UserPreferences { inlay_hints: preferences, ..Default::default() }
}

/// Rust-only: the body every Go test of this file shares after its content: create the fourslash test, open
/// `/test.ts` and verify its inlay hints with function-like return type hints on (Go's `defer done()` at the end).
fn run_inlay_hints_case(t: &T, content: &str) {
    ets_effecttest::init_hooks();
    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.go_to_file(t, "/test.ts");
    verify_local_baseline_inlay_hints(
        t,
        &mut f,
        content,
        "/test.ts",
        Some(new_inlay_hint_user_preferences(InlayHintsPreferences {
            include_inlay_function_like_return_type_hints: Tristate::True,
            ..Default::default()
        })),
    );
    f.done(t);
}

// go test: internal/effecttest/inlay_hints_test.go:14 TestEffectInlayHintsGenSuppression 0d8a19b81ad0dd95
fn test_effect_inlay_hints_gen_suppression(t: &T) {
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
        "name": "@effect/language-service",
        "inlays": true
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sample = Effect.gen(function*() {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#;

    run_inlay_hints_case(t, content);
}

// go test: internal/effecttest/inlay_hints_test.go:56 TestEffectInlayHintsFnSuppression 229611f7c2579807
fn test_effect_inlay_hints_fn_suppression(t: &T) {
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
        "name": "@effect/language-service",
        "inlays": true
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"

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
})"#;

    run_inlay_hints_case(t, content);
}

// go test: internal/effecttest/inlay_hints_test.go:101 TestEffectInlayHintsFnUntracedSuppression d991e2b22634e820
fn test_effect_inlay_hints_fn_untraced_suppression(t: &T) {
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
        "name": "@effect/language-service",
        "inlays": true
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sampleFnUntraced = Effect.fnUntraced(function*(_: boolean) {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#;

    run_inlay_hints_case(t, content);
}

// go test: internal/effecttest/inlay_hints_test.go:143 TestEffectInlayHintsNonEffectNotSuppressed 6a45aa47ecea81e2
fn test_effect_inlay_hints_non_effect_not_suppressed(t: &T) {
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
        "name": "@effect/language-service",
        "inlays": true
      }
    ]
  }
}
// @Filename: /test.ts
export function standardShouldAppear() {
  return 42
}"#;

    run_inlay_hints_case(t, content);
}

// go test: internal/effecttest/inlay_hints_test.go:175 TestEffectInlayHintsDisabledPassthrough 1f8088bfb2c13195
fn test_effect_inlay_hints_disabled_passthrough(t: &T) {
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
// @Filename: /test.ts
import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sample = Effect.gen(function*() {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})"#;

    run_inlay_hints_case(t, content);
}

/// Rust-only: the trials of this file, one per Go test, named like it (the baseline file name comes from the name).
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case(
            "TestEffectInlayHintsGenSuppression".to_string(),
            test_effect_inlay_hints_gen_suppression,
        ),
        run_case(
            "TestEffectInlayHintsFnSuppression".to_string(),
            test_effect_inlay_hints_fn_suppression,
        ),
        run_case(
            "TestEffectInlayHintsFnUntracedSuppression".to_string(),
            test_effect_inlay_hints_fn_untraced_suppression,
        ),
        run_case(
            "TestEffectInlayHintsNonEffectNotSuppressed".to_string(),
            test_effect_inlay_hints_non_effect_not_suppressed,
        ),
        run_case(
            "TestEffectInlayHintsDisabledPassthrough".to_string(),
            test_effect_inlay_hints_disabled_passthrough,
        ),
    ]
}
