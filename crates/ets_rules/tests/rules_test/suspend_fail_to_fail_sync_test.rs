//! Port of `internal/rules/suspend_fail_to_fail_sync_test.go`.

use ets_bundledeffect::EffectVersion;
use regex::Regex;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/rules/suspend_fail_to_fail_sync_test.go:13 TestSuspendFailToFailSyncShapes da5499ed557c04a6
#[test]
fn test_suspend_fail_to_fail_sync_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = r#"import * as Effect from "effect/Effect"
declare const error: Error
declare const defect: unknown
declare const args: [Error]
export const failCall = Effect.suspend(() => Effect.fail(error))
export const dieCall = Effect.suspend(() => Effect.die(defect))
export const functionThunk = Effect.suspend(function() {
  return Effect.fail(error)
})
export const asyncThunk = Effect.suspend((async () => Effect.fail(error)) as any)
export const generatorThunk = Effect.suspend(function*() {
  return Effect.fail(error)
} as any)
export const typeParameterThunk = Effect.suspend(<T>() => Effect.fail(error))
export const spreadArgument = Effect.suspend(() => Effect.fail(...args))
export const emptyReturn = Effect.suspend((() => {
  return
}) as any)
"#;
        // Rust: RE2's `\w` is ASCII (`[0-9A-Za-z_]`); the `regex` crate's is Unicode.
        let export_name = Regex::new(r"export const ([0-9A-Za-z_]+)").unwrap();
        let messages: FxHashMap<&str, &str> = FxHashMap::from_iter([
            (
                "failCall",
                "`Effect.failSync` builds this error lazily directly instead of wrapping `Effect.fail` in `Effect.suspend`.",
            ),
            (
                "dieCall",
                "`Effect.dieSync` builds this defect lazily directly instead of wrapping `Effect.die` in `Effect.suspend`.",
            ),
            (
                "functionThunk",
                "`Effect.failSync` builds this error lazily directly instead of wrapping `Effect.fail` in `Effect.suspend`.",
            ),
        ]);

        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let mut want = vec!["failCall", "dieCall", "functionThunk"];
                if version == EffectVersion::V4 {
                    // Effect v4 has no Effect.dieSync to suggest.
                    want.retain(|&name| name != "dieCall");
                }
                let preview =
                    evaluate_preview(t, version, source, &ets_rules::SUSPEND_FAIL_TO_FAIL_SYNC);
                let mut got: Vec<String> = Vec::new();
                for diagnostic in &preview.diagnostics {
                    let start = diagnostic.start as usize;
                    let line_start = preview.source_text[..start].rfind('\n').map_or(0, |i| i + 1);
                    let Some(m) = export_name.captures(&preview.source_text[line_start..start])
                    else {
                        t.fatal(format!("diagnostic at {start} is not on an export line"));
                    };
                    let name = m.get(1).unwrap().as_str().to_string();
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
