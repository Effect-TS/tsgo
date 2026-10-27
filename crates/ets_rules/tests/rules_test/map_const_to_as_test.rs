//! Port of `internal/rules/map_const_to_as_test.go`.

use ets_bundledeffect::EffectVersion;
use regex::Regex;

use crate::*;

// go test: internal/rules/map_const_to_as_test.go:13 TestMapConstToAsCallbackShapes
#[test]
fn test_map_const_to_as_callback_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
declare const ambient: string
declare let ambientLet: string
const enum Flag { On = 1 }
export const literal = source.pipe(Effect.map(() => "saved"))
export const negativeBigint = source.pipe(Effect.map(() => -1n))
export const constEnum = source.pipe(Effect.map(() => Flag.On))
export const ambientConst = source.pipe(Effect.map(() => ambient))
export const ambientMutable = source.pipe(Effect.map(() => ambientLet))
export const ambientLater = source.pipe(Effect.map(() => laterAmbient))
export const typeArguments = source.pipe(Effect.map<number, string>(() => "saved"))
export const restParameter = source.pipe(Effect.map((..._args: Array<number>) => "saved"))
export const asyncCallback = source.pipe(Effect.map(async () => "saved"))
export const notNegative = source.pipe(Effect.map(() => +1))
export const voidBody = source.pipe(Effect.map(() => undefined))
export const annotatedReturn = source.pipe(Effect.map((): "saved" => "saved"))
export const annotatedFunction = source.pipe(Effect.map(function(): number { return 1 }))
export const annotatedParameter = source.pipe(Effect.map((_: number) => "saved"))
declare const laterAmbient: string
"#;
        let want = ["literal", "negativeBigint", "constEnum", "ambientConst", "annotatedParameter"];
        let export_name = Regex::new(r"export const ([0-9A-Za-z_]+)").expect("valid regexp");

        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let preview = evaluate_preview(t, version, source, &ets_rules::MAP_CONST_TO_AS);
                let mut got: Vec<String> = Vec::new();
                for diagnostic in &preview.diagnostics {
                    if !diagnostic.text.contains(
                        "`Effect.as` expresses replacing the success value with a constant directly.",
                    ) {
                        t.fatal(format!("unexpected diagnostic text {:?}", diagnostic.text));
                    }
                    let start = diagnostic.start as usize;
                    let line_start =
                        preview.source_text[..start].rfind('\n').map_or(0, |index| index + 1);
                    let Some(m) = export_name.captures(&preview.source_text[line_start..start])
                    else {
                        t.fatal(format!("diagnostic at {} is not on an export line", diagnostic.start));
                    };
                    got.push(m[1].to_string());
                }
                if got != want {
                    t.fatal(format!("matched {got:?}, want {want:?}"));
                }
            });
        }
    });
}
