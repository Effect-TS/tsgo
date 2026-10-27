//! Port of `internal/rules/fail_cause_to_constructor_test.go`.

use ets_bundledeffect::EffectVersion;
use regex::Regex;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/rules/fail_cause_to_constructor_test.go:13 TestFailCauseToConstructorShapes
#[test]
fn test_fail_cause_to_constructor_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = r#"import { Cause, Effect, pipe } from "effect"
declare const error: Error
declare const defect: unknown
declare const wrap: (e: Error) => Error
export const failCall = Effect.failCause(Cause.fail(error))
export const dieCall = Effect.failCause(Cause.die(defect))
export const longerPipe = pipe(error, wrap, Cause.fail, Effect.failCause)
export const effectFnTrailing = Effect.fn(function*() {
  return yield* Effect.succeed(error)
}, Cause.fail, Effect.failCause)
export const lazyFail = Effect.failCauseSync(() => Cause.fail(error))
export const lazyDie = Effect.failCauseSync(() => Cause.die(defect))
export const lazyTypeArgument = Effect.failCauseSync(() => Cause.fail<Error | string>(error))
export const asyncThunk = Effect.failCauseSync(async () => Cause.fail(error))
export const generatorThunk = Effect.failCauseSync(function*() {
  return Cause.fail(error)
} as any)
export const parameterThunk = Effect.failCauseSync(((_?: unknown) => Cause.fail(error)) as () => Cause.Cause<Error>)
"#;
        // Rust: RE2's `\w` is ASCII (`[0-9A-Za-z_]`); the `regex` crate's is Unicode.
        let export_name = Regex::new(r"export const ([0-9A-Za-z_]+)").unwrap();
        let messages: FxHashMap<&str, &str> = FxHashMap::from_iter([
            (
                "failCall",
                "`Effect.fail` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCause`.",
            ),
            (
                "dieCall",
                "`Effect.die` expresses this failure directly instead of wrapping `Cause.die` in `Effect.failCause`.",
            ),
            (
                "longerPipe",
                "`Effect.fail` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCause`.",
            ),
            (
                "effectFnTrailing",
                "`Effect.fail` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCause`.",
            ),
            (
                "lazyFail",
                "`Effect.failSync` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCauseSync`.",
            ),
            (
                "lazyDie",
                "`Effect.dieSync` expresses this failure directly instead of wrapping `Cause.die` in `Effect.failCauseSync`.",
            ),
            (
                "lazyTypeArgument",
                "`Effect.failSync` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCauseSync`.",
            ),
        ]);

        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let mut want = vec![
                    "failCall",
                    "dieCall",
                    "longerPipe",
                    "effectFnTrailing",
                    "lazyFail",
                    "lazyDie",
                    "lazyTypeArgument",
                ];
                if version == EffectVersion::V4 {
                    // Effect v4 has no Effect.dieSync to suggest.
                    want.retain(|&name| name != "lazyDie");
                }
                let preview =
                    evaluate_preview(t, version, source, &ets_rules::FAIL_CAUSE_TO_CONSTRUCTOR);
                let mut got: Vec<String> = Vec::new();
                for diagnostic in &preview.diagnostics {
                    let start = diagnostic.start as usize;
                    let line_start = preview.source_text[..start].rfind('\n').map_or(0, |i| i + 1);
                    let name;
                    if let Some(m) = export_name.captures(&preview.source_text[line_start..start]) {
                        name = m.get(1).unwrap().as_str().to_string();
                    } else if preview.source_text[line_start..]
                        .starts_with("}, Cause.fail, Effect.failCause)")
                    {
                        name = "effectFnTrailing".to_string();
                    } else {
                        t.fatal(format!("diagnostic at {start} is not on an export line"));
                    }
                    let message = messages.get(name.as_str()).copied().unwrap_or_default();
                    if !diagnostic.text.contains(message) {
                        t.fatal(format!(
                            "{name}: unexpected diagnostic text {:?}",
                            diagnostic.text
                        ));
                    }
                    got.push(name);
                }
                if got != want {
                    t.fatal(format!("matched {got:?}, want {want:?}"));
                }
            });
        }
    });
}
