// Ported from effect-tsgo's internal/typeparser/api_stability_inheritance_test.go (see helpers_test.rs).

use crate::api_stability_test::*;
use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/api_stability_inheritance_test.go:5 TestApiStabilityInheritanceMinimumAndCacheIsolation
#[test]
fn test_api_stability_inheritance_minimum_and_cache_isolation() {
    run(|t| {
        t.parallel();
        for warm in [false, true] {
            let name = if warm { "warm" } else { "cold" };
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                    t,
                    &[(
                        "/.src/test.ts",
                        "/** @stability unstable */
interface Reportable {
/** @stability experimental */ test?: string }
declare global { interface Error extends Reportable {} }
export function augmented(e: Error) {}
export interface Expanded extends Error {}
export interface Ordinary extends Reportable {}
export function direct(e: Reportable) {}
",
                    )],
                );
                if warm {
                    prime_api_stability_checker(t, &mut compiled);
                }
                let mut tp = compiled.type_parser();
                let sf = files["/.src/test.ts"];
                // Repeat with opposite root orders so neither shared surfaces nor an
                // analysis-local result can erase a directly exposed dependency.
                for order in [
                    ["augmented", "Expanded", "direct", "Ordinary"],
                    ["Ordinary", "direct", "Expanded", "augmented"],
                ] {
                    for name in order {
                        let symbol = api_stability_test_export(t, tp.checker, sf, name);
                        if warm && name == "Expanded" {
                            let declared = tp.checker.get_declared_type_of_symbol_exported(symbol);
                            tp.checker.get_properties_of_type_exported(declared);
                        }
                        let used = tp.api_stability_used_by_symbol(symbol);
                        let mut want = ApiStabilityLevel::Stable;
                        if name == "direct" {
                            want = ApiStabilityLevel::Unstable;
                        }
                        if used.minimum != want || (warm && used.incomplete) {
                            t.error(ets_go::sprintf!(
                                "%s minimum=%v incomplete=%v deps=%v, want %v",
                                name,
                                used.minimum as i64,
                                used.incomplete,
                                api_stability_dependency_names(&used),
                                want as i64
                            ));
                        }
                        if name == "Expanded" {
                            // Also inspect its represented type after the compiler has
                            // flattened the inherited member table.
                            let typ = tp.checker.get_declared_type_of_symbol_exported(symbol);
                            tp.checker.get_properties_of_type_exported(typ);
                            let used = tp.api_stability_used_by_type(typ);
                            if used.incomplete || used.minimum != ApiStabilityLevel::Stable {
                                t.error(ets_go::sprintf!(
                                    "expanded type minimum=%v incomplete=%v deps=%v",
                                    used.minimum as i64,
                                    used.incomplete,
                                    api_stability_dependency_names(&used)
                                ));
                            }
                        }
                    }
                }
            });
        }
    });
}
