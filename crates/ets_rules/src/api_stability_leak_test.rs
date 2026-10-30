// Ported from effect-tsgo's internal/rules/api_stability_leak_test.go (helpers in
// api_stability_leak_regression_test.rs).

use ets_go::testing::T;
use ets_typeparser::get_symbol_of_declaration;

use crate::api_stability_leak_regression_test::*;
use crate::*;

// go test: internal/rules/api_stability_leak_test.go:24 compileApiStabilitySource ded003f532cebd39
#[expect(
    dead_code,
    reason = "Go's only remaining caller is BenchmarkStabilityApiUsageContextualProperties (porting/skipped.tsv)"
)]
pub(crate) fn compile_api_stability_source(t: &T, source: &str) -> ApiStabilityCompiled {
    t.helper();
    compile_api_stability_source_primed(t, source, true)
}

// go test: internal/rules/api_stability_leak_test.go:29 compileApiStabilitySourcePrimed 570954b7e98f200b
// Rust: the same program as `compile_api_stability_files_primed` over the one file `/.src/test.ts`.
pub(crate) fn compile_api_stability_source_primed(
    t: &T,
    source: &str,
    prime: bool,
) -> ApiStabilityCompiled {
    t.helper();
    compile_api_stability_files_primed(t, &files(&[("test.ts", source)]), prime)
}

// go test: internal/rules/api_stability_leak_test.go:70 TestApiStabilityLeakExportOrderIsDeterministic 620c8ede9b1bce3b
// Rust: tsrs's exports hold no nil, so Go's nil-symbol checks cannot fail and drop.
#[test]
fn test_api_stability_leak_export_order_is_deterministic() {
    run(|t| {
        t.parallel();

        let mut compiled = compile_api_stability_files_primed(
            t,
            &files(&[(
                "test.ts",
                r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
type P = string
export declare const zeta: E
export declare const alpha: P
export interface Middle { value: E }
export declare const beta: { value: P }
export declare const omega: Middle
",
            )]),
            true,
        );
        compiled.with_rule_ctx(|rule_ctx| {
            let source_file = rule_ctx.source_file;
            let Some(module) = get_symbol_of_declaration(rule_ctx.checker(), source_file.as_node())
            else {
                t.fatal("module symbol missing");
            };
            let all = rule_ctx.checker().get_exports_of_module_exported(module);
            if all.len() < 5 {
                t.fatal(format!("fixture should export several symbols, got {}", all.len()));
            }
            let mut want_names: Vec<&str> = all.iter().map(|symbol| symbol.name()).collect();
            want_names.sort_unstable();

            let mut first: Option<Vec<&str>> = None;
            for _ in 0..64 {
                let ordered = sorted_api_stability_exports(rule_ctx, module);
                if ordered.len() != all.len() {
                    t.fatal(format!(
                        "export sorting dropped symbols: got {}, want {}",
                        ordered.len(),
                        all.len()
                    ));
                }
                let names: Vec<&str> = ordered.iter().map(|symbol| symbol.name()).collect();
                if !names.is_sorted() {
                    t.fatal(format!("export order is not canonical: {names:?}"));
                }
                let Some(first) = &first else {
                    first = Some(names);
                    continue;
                };
                if *first != names {
                    t.fatal(format!(
                        "export order changed between invocations:\nfirst={first:?}\nlater={names:?}"
                    ));
                }
            }
            if first.as_ref() != Some(&want_names) {
                t.fatal(format!(
                    "canonical order does not match the sorted export set:\ngot={first:?}\nwant={want_names:?}"
                ));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_test.go:130 TestApiStabilityLeakRunsWithoutInstantiation 862a05efa846acf7
#[test]
fn test_api_stability_leak_runs_without_instantiation() {
    run(|t| {
        t.parallel();

        let mut alias_chain = files(&[
            ("test.ts", r#"export { Leaky } from "./barrel0""#),
            (
                "dep.ts",
                r"/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
",
            ),
        ]);
        for index in 0..40 {
            let mut next = format!("barrel{}", index + 1);
            if index == 39 {
                next = "dep".to_string();
            }
            alias_chain.push((
                format!("barrel{index}.ts"),
                format!("export {{ Leaky }} from {:?}", format!("./{next}")),
            ));
        }

        let tests = vec![
            case(
                "internal export filtering",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
declare const e: E
/** @internal */
export declare const hidden: E
export declare const shown: E
",
                )]),
                &["`shown` exposes `E`"],
                &["`hidden` exposes"],
            ),
            case(
                "import equals filtering",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
declare const e: E
namespace Impl {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}
export import Hidden = Impl.Hidden
export import Shown = Impl.Shown
",
                )]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            case(
                "self-referential surface",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
export interface Node { next: Node; payload: E }
",
                )]),
                &["`Node` exposes `E`"],
                &[],
            ),
            case("40-hop alias chain", alias_chain, &["`Leaky` exposes `E`"], &[]),
        ];

        for test in &tests {
            t.run(&test.name, |t| {
                t.parallel();
                assert_api_stability_sources_parse(t, &test.files);
                let mut compiled = compile_api_stability_files(t, &test.files);
                compiled.with_rule_ctx(|rule_ctx| {
                    let before = rule_ctx.checker().total_instantiation_count;
                    let first = run_api_stability_messages(t, rule_ctx);
                    let delta = rule_ctx.checker().total_instantiation_count - before;
                    let second = run_api_stability_messages(t, rule_ctx);
                    let delta2 = rule_ctx.checker().total_instantiation_count - before - delta;

                    if delta != 0 || delta2 != 0 {
                        t.error(format!("rule instantiated {delta} then {delta2} types, want 0"));
                    }
                    let globals = rule_ctx.checker().get_global_diagnostics();
                    if !globals.is_empty() {
                        let globals: Vec<String> =
                            globals.iter().map(|diagnostic| diagnostic.localize()).collect();
                        t.error(format!("rule produced global diagnostics: {globals:?}"));
                    }
                    if first != second {
                        t.error(format!(
                            "repeated runs disagree:\nfirst={first:?}\nsecond={second:?}"
                        ));
                    }
                    for want in &test.want {
                        if !contains_api_stability_message(t, &first, want) {
                            t.error(format!("missing diagnostic {want:?} in {first:?}"));
                        }
                    }
                    for not_want in &test.not_want {
                        if contains_api_stability_message(t, &first, not_want) {
                            t.error(format!("unexpected diagnostic {not_want:?} in {first:?}"));
                        }
                    }
                });
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_test.go:228 TestApiStabilityLeakSurfaceSharedSnapshotReuse 95c1a929eec334a6
#[test]
fn test_api_stability_leak_surface_shared_snapshot_reuse() {
    run(|t| {
        t.parallel();

        let files = single_api_stability_file(
            r"/** @stability unstable */
export interface ATagged { leak: E }
export interface ZHolder { child: ATagged }
/** @stability unstable */
export interface ZTagged { leak: E }
export interface AHolder { child: ZTagged }
/** @stability stable */
export interface StableBox<T> { value: T }
export interface BoxHolder { box: StableBox<E> }
export declare function makeTagged(): ZTagged
",
        );
        let mut compiled = compile_api_stability_files(t, &files);
        compiled.with_rule_ctx(|rule_ctx| {
            let first = run_api_stability_messages(t, rule_ctx);
            let second = run_api_stability_messages(t, rule_ctx);
            let mut sorted_first = first.clone();
            let mut sorted_second = second;
            sorted_first.sort();
            sorted_second.sort();
            if sorted_first != sorted_second {
                t.fatal(format!(
                    "second run over the shared snapshots disagrees:\nfirst={sorted_first:?}\nsecond={sorted_second:?}"
                ));
            }

            for want in [
                "`ATagged` exposes `E`",
                "`ZTagged` exposes `E`",
                "`ZHolder` exposes `ATagged`, an unstable API.",
                "`AHolder` exposes `ZTagged`, an unstable API.",
                "`BoxHolder` exposes `E`",
                "`makeTagged` exposes `ZTagged`, an unstable API.",
            ] {
                if !contains_api_stability_message(t, &first, want) {
                    t.error(format!("missing diagnostic {want:?} in {first:?}"));
                }
            }
            for not_want in [
                "`ZHolder` exposes `E`",
                "`AHolder` exposes `E`",
                "`makeTagged` exposes `E`",
                "`ATagged` exposes `ATagged`",
                "`ZTagged` exposes `ZTagged`",
            ] {
                if contains_api_stability_message(t, &first, not_want) {
                    t.error(format!("unexpected diagnostic {not_want:?} in {first:?}"));
                }
            }
        });
    });
}
