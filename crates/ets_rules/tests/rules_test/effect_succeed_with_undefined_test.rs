//! Port of `internal/rules/effect_succeed_with_undefined_test.go`.

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;

use crate::*;

// go test: internal/rules/effect_succeed_with_undefined_test.go:12 unknownEffectVersionPrefix f7cfa5d92141ca00
// Rust: Go decodes package.json into `map[string]json.RawMessage` and re-encodes it (keys sorted, values compacted);
// here the values go through the test's Go-`any` encoder (`Json`), which keeps the same keys and values.
pub(crate) fn unknown_effect_version_prefix(t: &T) -> String {
    t.helper();
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
    format!("// @filename: /node_modules/effect/package.json\n{unknown_package}\n")
}

// go test: internal/rules/effect_succeed_with_undefined_test.go:31 TestEffectSucceedWithUndefinedUnknownVersion cc4fc29a2b2a6a2b
#[test]
fn test_effect_succeed_with_undefined_unknown_version() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = unknown_effect_version_prefix(t)
            + r#"// @filename: preview.ts
import { Effect } from "effect"
export const a = Effect.succeed(undefined)
export const b = Effect.succeed(void 0)
export const c = Effect.succeed<void>(undefined)
export const d = Effect.succeed<number | undefined>(undefined)
"#;
        let got = evaluate_preview_without_hooks(
            t,
            EffectVersion::V4,
            &source,
            &ets_rules::EFFECT_SUCCEED_WITH_UNDEFINED,
        )
        .diagnostics
        .len();
        if got != 0 {
            t.fatal(format!("expected no effectSucceedWithUndefined diagnostics, got {got}"));
        }
        let got = evaluate_preview_without_hooks(
            t,
            EffectVersion::V4,
            &source,
            &ets_rules::EFFECT_SUCCEED_WITH_VOID,
        )
        .diagnostics
        .len();
        if got != 3 {
            t.fatal(format!("expected 3 effectSucceedWithVoid diagnostics, got {got}"));
        }
    });
}
