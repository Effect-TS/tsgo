//! Port of `internal/rules/effect_sleep_zero_to_yield_now_test.go`.

use ets_bundledeffect::EffectVersion;

use crate::*;

// go test: internal/rules/effect_sleep_zero_to_yield_now_test.go:12 TestEffectSleepZeroToYieldNowVersionDetection
//
// Rust: Go decodes package.json into `map[string]json.RawMessage` and re-encodes it (keys sorted, values compacted);
// here the values go through the test's Go-`any` encoder (`Json`), which keeps the same keys and values.
#[test]
fn test_effect_sleep_zero_to_yield_now_version_detection() {
    run_on_checker_stack(|t| {
        t.parallel();
        let mut testfs = ets_bundledeffect::TestFS::default();
        if let Err(err) = ets_bundledeffect::mount_effect(EffectVersion::V4, &mut testfs) {
            t.fatal(err.to_string());
        }
        let package_file = &testfs["/node_modules/effect/package.json"];
        let package_json: serde_json::Value = match serde_json::from_slice(&package_file.data) {
            Ok(package_json) => package_json,
            Err(err) => t.fatal(err.to_string()),
        };
        let Json::Object(mut package_json) = Json::from_decoded(&package_json) else {
            t.fatal("package.json is not an object");
        };
        package_json.retain(|(key, _)| key != "version");
        let unknown_package = Json::Object(package_json).marshal();
        const SOURCE: &str = r#"// @filename: preview.ts
import { Effect } from "effect"
export const sleep = Effect.sleep(0)
"#;
        let unknown =
            format!("// @filename: /node_modules/effect/package.json\n{unknown_package}\n{SOURCE}");
        let mixed = format!(
            r#"// @filename: /node_modules/nested/package.json
{{ "name": "nested", "version": "1.0.0", "types": "index.d.ts" }}
// @filename: /node_modules/nested/index.d.ts
export {{ marker }} from "effect"
// @filename: /node_modules/nested/node_modules/effect/package.json
{{ "name": "effect", "version": "3.19.19", "types": "index.d.ts" }}
// @filename: /node_modules/nested/node_modules/effect/index.d.ts
export declare const marker: unique symbol
{SOURCE}import {{ marker }} from "nested"
export const mixedPackage = marker
"#
        );
        for (name, source, want) in
            [("known", SOURCE.to_string(), 1), ("unknown", unknown, 0), ("mixed", mixed, 0)]
        {
            t.run(name, |t| {
                t.parallel();
                let preview = evaluate_preview(
                    t,
                    EffectVersion::V4,
                    &source,
                    &ets_rules::EFFECT_SLEEP_ZERO_TO_YIELD_NOW,
                );
                if preview.diagnostics.len() != want {
                    t.fatal(format!(
                        "expected {want} sleep-zero suggestions, got {}",
                        preview.diagnostics.len()
                    ));
                }
            });
        }
    });
}
