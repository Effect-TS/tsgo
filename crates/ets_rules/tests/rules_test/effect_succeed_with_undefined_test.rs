//! Port of `internal/rules/effect_succeed_with_undefined_test.go`.

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;

use crate::*;

// go test: internal/rules/effect_succeed_with_undefined_test.go:14 unknownEffectVersionPrefix b3502539187cff70
// unknownEffectVersionPrefix returns a preview prefix that replaces the bundled v4
// effect package.json with one that has no version, so the Effect version is unknown.
//
// Rust: Go decodes package.json into `map[string]json.RawMessage` and re-encodes it (keys sorted, values compacted);
// here the values go through the test's Go-`any` encoder (`Json`), which keeps the same keys and values.
fn unknown_effect_version_prefix(t: &T) -> String {
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

// go test: internal/rules/effect_succeed_with_undefined_test.go:33 TestEffectSucceedWithUndefinedCallShapes 2f6cee39c84e216b
#[test]
fn test_effect_succeed_with_undefined_call_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        const HEADER: &str = "// @filename: preview.ts\nimport { Effect, pipe } from \"effect\"\n";
        for (name, source, want) in [
            ("plain", "export const a = Effect.succeed(undefined)\n", 1),
            (
                "data-first argument",
                "export const a = Effect.andThen(Effect.void, Effect.succeed(undefined))\n",
                1,
            ),
            ("pipe subject", "export const a = pipe(Effect.succeed(undefined), Effect.as(1))\n", 1),
            (
                "Effect.fn body",
                "export const a = Effect.fn(function*() { return yield* Effect.succeed(undefined) })\n",
                1,
            ),
            (
                "Effect.fnUntraced body",
                "export const a = Effect.fnUntraced(function*() { return yield* Effect.succeed(undefined) })\n",
                1,
            ),
            (
                "undefined-typed shadowing parameter",
                "export const a = (undefined: undefined) => Effect.succeed(undefined)\n",
                0,
            ),
            ("point-free pipe", "export const a = pipe(undefined, Effect.succeed)\n", 1),
            (
                "point-free pipe with steps",
                "export const a = pipe((void 0), Effect.succeed, Effect.as(1))\n",
                1,
            ),
            (
                "point-free pipe as a data-first argument",
                "export const a = Effect.andThen(Effect.void, pipe(undefined, Effect.succeed))\n",
                1,
            ),
            ("void 0", "export const a = Effect.succeed(void 0)\n", 1),
            (
                "explicit void type argument",
                "export const a = Effect.succeed<void>(undefined)\n",
                0,
            ),
        ] {
            t.run(name, |t| {
                t.parallel();
                let preview = evaluate_preview(
                    t,
                    EffectVersion::V4,
                    &format!("{HEADER}{source}"),
                    &ets_rules::EFFECT_SUCCEED_WITH_UNDEFINED,
                );
                if preview.diagnostics.len() != want {
                    t.fatal(format!(
                        "expected {want} effectSucceedWithUndefined diagnostics, got {}",
                        preview.diagnostics.len()
                    ));
                }
            });
        }
    });
}

// go test: internal/rules/effect_succeed_with_undefined_test.go:63 TestEffectSucceedWithUndefinedVersionGate 74bc59e166ed8883
#[test]
fn test_effect_succeed_with_undefined_version_gate() {
    run_on_checker_stack(|t| {
        t.parallel();
        const SOURCE: &str = r#"// @filename: preview.ts
import { Effect } from "effect"
export const a = Effect.succeed(undefined)
export const b = Effect.succeed(void 0)
export const c = Effect.succeed<void>(undefined)
export const d = Effect.succeed<number | undefined>(undefined)
"#;
        let unknown = unknown_effect_version_prefix(t);
        for (name, version, prefix, want_undefined, want_void) in [
            ("v4", EffectVersion::V4, "", 2, 1),
            ("v3", EffectVersion::V3, "", 0, 3),
            ("unknown", EffectVersion::V4, unknown.as_str(), 0, 3),
        ] {
            t.run(name, |t| {
                t.parallel();
                let undefined_preview = evaluate_preview(
                    t,
                    version,
                    &format!("{prefix}{SOURCE}"),
                    &ets_rules::EFFECT_SUCCEED_WITH_UNDEFINED,
                );
                if undefined_preview.diagnostics.len() != want_undefined {
                    t.fatal(format!(
                        "expected {want_undefined} effectSucceedWithUndefined diagnostics, got {}",
                        undefined_preview.diagnostics.len()
                    ));
                }
                let void_preview = evaluate_preview(
                    t,
                    version,
                    &format!("{prefix}{SOURCE}"),
                    &ets_rules::EFFECT_SUCCEED_WITH_VOID,
                );
                if void_preview.diagnostics.len() != want_void {
                    t.fatal(format!(
                        "expected {want_void} effectSucceedWithVoid diagnostics, got {}",
                        void_preview.diagnostics.len()
                    ));
                }
            });
        }
    });
}
