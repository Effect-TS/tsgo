//! Port of `internal/rules/map_map_error_to_map_both_test.go`.

use ets_bundledeffect::EffectVersion;

use crate::*;

/// Rust-only: one entry of the Go test's anonymous `expected` struct slice.
struct Expected {
    location: &'static str,
    first: &'static str,
    second: &'static str,
    replacement: &'static str,
}

// go test: internal/rules/map_map_error_to_map_both_test.go:11 TestMapMapErrorToMapBothMessagesAndLocations
#[test]
fn test_map_map_error_to_map_both_messages_and_locations() {
    run_on_checker_stack(|t| {
        t.parallel();
        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let mut source = r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const forward = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
export const reverse = source.pipe(Effect.mapError((e) => e.length), Effect.map((n) => n + 1))
export const nested = Effect.mapError(Effect.map(source, (n) => n + 1), (e) => e.length)
"#
                .to_string();
                let mut expected = vec![
                    Expected {
                        location: "Effect.mapError",
                        first: "map",
                        second: "mapError",
                        replacement: "mapBoth",
                    },
                    Expected {
                        location: "Effect.map",
                        first: "mapError",
                        second: "map",
                        replacement: "mapBoth",
                    },
                    Expected {
                        location: "Effect.mapError",
                        first: "map",
                        second: "mapError",
                        replacement: "mapBoth",
                    },
                ];
                if version == EffectVersion::V4 {
                    source += "export const eager = source.pipe(Effect.mapErrorEager((e) => e.length), Effect.mapEager((n) => n + 1))\n";
                    expected.push(Expected {
                        location: "Effect.mapEager",
                        first: "mapErrorEager",
                        second: "mapEager",
                        replacement: "mapBothEager",
                    });
                }
                let preview =
                    evaluate_preview(t, version, &source, &ets_rules::MAP_MAP_ERROR_TO_MAP_BOTH);
                if preview.diagnostics.len() != expected.len() {
                    t.fatal(format!(
                        "expected {} diagnostics, got {}",
                        expected.len(),
                        preview.diagnostics.len()
                    ));
                }
                let lines: Vec<&str> = source.split('\n').collect();
                for (i, diagnostic) in preview.diagnostics.iter().enumerate() {
                    let want = &expected[i];
                    let got = &preview.source_text
                        [diagnostic.start as usize..diagnostic.end as usize];
                    if got != want.location {
                        t.error(format!(
                            "diagnostic {i} is on {got:?}, want {:?}",
                            want.location
                        ));
                    }
                    if !lines[i + 2].contains(got) {
                        t.error(format!("diagnostic {i} is not on line {:?}", lines[i + 2]));
                    }
                    let message = "`Effect.".to_string()
                        + want.first
                        + "` + `Effect."
                        + want.second
                        + "` transform the success and failure channels one step at a time; `Effect."
                        + want.replacement
                        + "` expresses both transformations in a single step.";
                    if !diagnostic.text.contains(&message) {
                        t.error(format!(
                            "diagnostic {i} message = {:?}, want {message:?}",
                            diagnostic.text
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/rules/map_map_error_to_map_both_test.go:53 TestMapMapErrorToMapBothBoundaries
#[test]
fn test_map_map_error_to_map_both_boundaries() {
    run_on_checker_stack(|t| {
        t.parallel();
        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let preview = evaluate_preview(
                    t,
                    version,
                    r#"import { Effect, pipe } from "effect"
declare const source: Effect.Effect<number, string>
declare const callbacks: [(n: number) => number]
export const control = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
export const nestedPipeCalls = pipe(pipe(source, Effect.map((n) => n + 1)), Effect.mapError((e) => e.length))
export const emptyPipeSubject = Effect.map(Effect.mapError(source, (e) => e.length).pipe(), (n) => n + 1)
export const curried = Effect.map((n: number) => n + 1)(Effect.mapError((e: string) => e.length)(source))
export const untracedPipeline = Effect.fnUntraced(function*() {
  return yield* source
}, Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
export const spreadArgument = source.pipe(Effect.map(...callbacks), Effect.mapError((e) => e.length))
export const separatedAcrossPipes = source.pipe(Effect.map((n) => n + 1), Effect.asVoid).pipe(Effect.mapError((e) => e.length))
export const separateFlows = source.pipe(Effect.map((n) => Effect.succeed(n).pipe(Effect.mapError(() => 0))))
"#,
                    &ets_rules::MAP_MAP_ERROR_TO_MAP_BOTH,
                );
                // Adjacency anywhere in one flow counts; separated steps, spread
                // arguments and steps of different flows do not.
                let want = ["control", "nestedPipeCalls", "emptyPipeSubject", "curried", "untracedPipeline"];
                let mut got: Vec<&str> = Vec::new();
                for diagnostic in &preview.diagnostics {
                    let before = &preview.source_text[..diagnostic.start as usize];
                    let from = before.rfind("export const ").map_or(-1, |index| index as i64)
                        + "export const ".len() as i64;
                    let declaration = &preview.source_text[from as usize..];
                    let name = declaration.split_once(' ').map_or(declaration, |(name, _)| name);
                    got.push(name);
                }
                if got.join(",") != want.join(",") {
                    t.fatal(format!("diagnostics in {got:?}, want {want:?}"));
                }
            });
        }
    });
}
