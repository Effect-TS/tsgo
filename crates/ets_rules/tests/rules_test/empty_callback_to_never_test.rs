//! Port of `internal/rules/empty_callback_to_never_test.go`.

use ets_bundledeffect::EffectVersion;

use crate::effect_succeed_with_undefined_test::unknown_effect_version_prefix;
use crate::*;

// go test: internal/rules/empty_callback_to_never_test.go:10 TestEmptyCallbackToNeverVersionDetection cd5d31bdefe92aa5
#[test]
fn test_empty_callback_to_never_version_detection() {
    run_on_checker_stack(|t| {
        t.parallel();
        const SOURCE: &str = r#"// @filename: preview.ts
import { Effect } from "effect"
export const park = Effect.callback<never>(() => {})
"#;
        for (name, prefix, suffix, want) in [
            ("known", String::new(), "", 1),
            ("unknown", unknown_effect_version_prefix(t), "", 0),
            (
                "mixed",
                r#"// @filename: /node_modules/nested/package.json
{ "name": "nested", "version": "1.0.0", "types": "index.d.ts" }
// @filename: /node_modules/nested/index.d.ts
export { marker } from "effect"
// @filename: /node_modules/nested/node_modules/effect/package.json
{ "name": "effect", "version": "3.19.19", "types": "index.d.ts" }
// @filename: /node_modules/nested/node_modules/effect/index.d.ts
export declare const marker: unique symbol
"#
                .to_string(),
                "import { marker } from \"nested\"\nexport const mixedPackage = marker\n",
                0,
            ),
        ] {
            t.run(name, |t| {
                t.parallel();
                let text = format!("{prefix}{SOURCE}{suffix}");
                let got = evaluate_preview_without_hooks(
                    t,
                    EffectVersion::V4,
                    &text,
                    &ets_rules::EMPTY_CALLBACK_TO_NEVER,
                )
                .diagnostics
                .len();
                if got != want {
                    t.fatal(format!("expected {want} emptyCallbackToNever diagnostics, got {got}"));
                }
            });
        }
    });
}
