//! Port of `internal/rules/effect_sleep_zero_to_yield_now_test.go`.

use ets_bundledeffect::EffectVersion;

use crate::effect_succeed_with_undefined_test::unknown_effect_version_prefix;
use crate::*;

// go test: internal/rules/effect_sleep_zero_to_yield_now_test.go:10 TestEffectSleepZeroToYieldNowUnknownVersion 190e87e5f3863145
#[test]
fn test_effect_sleep_zero_to_yield_now_unknown_version() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = unknown_effect_version_prefix(t)
            + r#"// @filename: preview.ts
import { Effect } from "effect"
export const sleep = Effect.sleep(0)
"#;
        let preview = evaluate_preview_without_hooks(
            t,
            EffectVersion::V4,
            &source,
            &ets_rules::EFFECT_SLEEP_ZERO_TO_YIELD_NOW,
        );
        if !preview.diagnostics.is_empty() {
            t.fatal(format!(
                "expected no sleep-zero suggestions, got {}",
                preview.diagnostics.len()
            ));
        }
    });
}
