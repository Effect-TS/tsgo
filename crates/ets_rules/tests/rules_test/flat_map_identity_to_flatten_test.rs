//! Port of `internal/rules/flat_map_identity_to_flatten_test.go`.

use ets_bundledeffect::EffectVersion;

use crate::*;

// go test: internal/rules/flat_map_identity_to_flatten_test.go:11 TestFlatMapIdentityToFlattenCallbackShapes
//
// Rust: Go's `%+v` of the diagnostics is spelled out as `[{Start:.. End:.. Text:..} ...]`.
#[test]
fn test_flat_map_identity_to_flatten_callback_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let preview = evaluate_preview(
                    t,
                    version,
                    r#"import { Channel, Effect, identity } from "effect"
import { flatMap } from "effect/Effect"
declare const source: Effect.Effect<Effect.Effect<string>>
declare const fallback: Effect.Effect<string>
export const valid = source.pipe(Effect.flatMap((inner) => inner))
export const namedImport = source.pipe(flatMap((inner) => inner))
export const returnType = source.pipe(Effect.flatMap((inner): Effect.Effect<string> => inner))
export const widened = source.pipe(Effect.flatMap((inner: Effect.Effect<string, unknown>) => inner))
export const typeArguments = source.pipe(Effect.flatMap<Effect.Effect<string>, string, never, never>(identity))
export const asyncCallback = source.pipe(Effect.flatMap(async (inner) => inner))
export const twoParameters = source.pipe(Effect.flatMap((inner, _extra?: number) => inner))
export const restParameter = source.pipe(Effect.flatMap((...inner: [Effect.Effect<string>]) => inner))
export const initializer = source.pipe(Effect.flatMap((inner = fallback) => inner))
export const optionalParameter = source.pipe(Effect.flatMap((inner?: Effect.Effect<string>) => inner))
export const destructured = source.pipe(Effect.flatMap(({ pipe }) => pipe()))
export const channelIdentity = source.pipe(Effect.flatMap(Channel.identity))
export const functionExpression = source.pipe(Effect.flatMap(function(inner) { return inner }))
export const dataLastIdentity = Effect.flatMap(identity)(source)
export const returnAnnotatedAny = source.pipe(Effect.flatMap((inner): Effect.Effect<any> => inner))
const plainAlias = identity
export const untypedAlias = source.pipe(Effect.flatMap(plainAlias))
const exactAlias: (x: Effect.Effect<string>) => Effect.Effect<string> = identity
export const typedAliasExact = source.pipe(Effect.flatMap(exactAlias))
const unwrapAnySuccess: (x: Effect.Effect<string>) => Effect.Effect<any> = identity
export const typedAliasAnySuccess = source.pipe(Effect.flatMap(unwrapAnySuccess))
export const asyncFunctionExpression = source.pipe(Effect.flatMap(async function(inner) { return inner }))
export const generatorFunctionExpression = source.pipe(Effect.flatMap(function*(inner) { return inner }))
export const typedFunctionExpression = source.pipe(Effect.flatMap(function(inner: Effect.Effect<string, unknown>) { return inner }))
"#,
                    &ets_rules::FLAT_MAP_IDENTITY_TO_FLATTEN,
                );
                if preview.diagnostics.len() != 3 {
                    let diagnostics: Vec<String> = preview
                        .diagnostics
                        .iter()
                        .map(|d| format!("{{Start:{} End:{} Text:{}}}", d.start, d.end, d.text))
                        .collect();
                    t.fatal(format!(
                        "expected only the valid, named-import and function-expression callbacks to match, got {} diagnostics: [{}]",
                        preview.diagnostics.len(),
                        diagnostics.join(" ")
                    ));
                }
                for (i, want) in ["Effect.flatMap", "flatMap", "Effect.flatMap"].into_iter().enumerate() {
                    let diagnostic = &preview.diagnostics[i];
                    let got =
                        &preview.source_text[diagnostic.start as usize..diagnostic.end as usize];
                    if got != want {
                        t.error(format!("diagnostic {i} covers {got:?}, want {want:?}"));
                    }
                }
                for (i, want) in [4, 5, 16].into_iter().enumerate() {
                    let start = preview.diagnostics[i].start as usize;
                    let line = preview.source_text[..start].matches('\n').count();
                    if line != want {
                        t.error(format!("diagnostic {i} on line {line}, want {want}"));
                    }
                }
            });
        }
    });
}
