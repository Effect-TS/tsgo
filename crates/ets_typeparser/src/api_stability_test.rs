// Ported from effect-tsgo's internal/typeparser/api_stability_test.go (see helpers_test.rs).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ets_go::testing::T;
use ets_shim::ast::{INTERNAL_SYMBOL_NAME_PREFIX, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::{
    Checker, Signature, SignatureKind, register_after_check_source_file_callback,
};
use ets_shim::core::P;
use ets_shim::core::context::Context;
use rustc_hash::FxHashSet;

use crate::api_stability_surface_test::api_stability_shared_type_entry;
use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/api_stability_test.go:15 apiStabilityTestExport 5d6383b4dfc316e6
// apiStabilityTestExport looks up a module export by name so the cache tests can
// exercise the public TypeParser API the same way the diagnostic does.
pub(crate) fn api_stability_test_export(
    t: &T,
    c: &mut Checker,
    sf: P<SourceFile>,
    name: &str,
) -> P<Symbol> {
    t.helper();
    let Some(module) = get_symbol_of_declaration(c, sf.as_node()) else {
        t.fatal(ets_go::sprintf!("no module symbol for %s", sf.file_name()));
    };
    for symbol in c.get_exports_of_module_exported(module) {
        if symbol.name() == name {
            return symbol;
        }
    }
    t.fatal(ets_go::sprintf!("export %q not found in %s", name, sf.file_name()));
}

// go test: internal/typeparser/api_stability_test.go:30 apiStabilityDependencyNames 742ac281dfa30939
pub(crate) fn api_stability_dependency_names(used: &ApiStabilityUsed) -> Vec<String> {
    let mut names = Vec::with_capacity(used.dependencies.len());
    for dependency in &used.dependencies {
        names.push(api_stability_dependency_name(dependency));
    }
    names
}

// go test: internal/typeparser/api_stability_test.go:38 containsDependency 55b4e1a3bde36dac
pub(crate) fn contains_dependency(used: &ApiStabilityUsed, name: &str) -> bool {
    for dependency in &used.dependencies {
        if api_stability_dependency_name(dependency) == name {
            return true;
        }
    }
    false
}

// go test: internal/typeparser/api_stability_test.go:52 primeApiStabilityChecker 1c6161e9ed07d19a
// primeApiStabilityChecker type checks the program against the checker, the
// way the normal rule lifecycle does before a diagnostic run. Only then are the
// public types the analysis reads through its cached-materialization peeks
// available; an unprimed checker models a first run whose type-driven surfaces
// must stay incomplete (and are never reported as stable).
//
// Rust: Go runs `program.GetSemanticDiagnostics(ctx, nil)`, which checks every file the program does not skip
// with the program's (single) checker. The test already holds that checker's handle, and asking the program would
// take it again (a deadlock in tsrs), so the same files are checked through the held handle.
pub(crate) fn prime_api_stability_checker(t: &T, compiled: &mut Compiled) {
    t.helper();
    let ctx = Context::background();
    for &file in compiled.program.get_source_files() {
        if compiled.program.skip_type_checking(file, false) {
            continue;
        }
        let _ = compiled.checker.get_diagnostics_exported(&ctx, file);
    }
}

/// Rust-only: Go's `%v` of the dependency names (failure messages only).
fn names(used: &ApiStabilityUsed) -> String {
    format!("{:?}", api_stability_dependency_names(used))
}

// go test: internal/typeparser/api_stability_test.go:66 TestApiStabilityDeclaredIndexInfosDoNotResolveComputedNames e6272d68610d1185
// TestApiStabilityDeclaredIndexInfosDoNotResolveComputedNames proves the
// declared index-info accessor inspects index signatures without resolving the
// symbol's late-bound member table: a cold interface carrying a recursively
// computed property name is inspected with zero checker instantiation and no
// compiler diagnostic.
#[test]
fn test_api_stability_declared_index_infos_do_not_resolve_computed_names() {
    run(|t| {
        t.parallel();

        let source = format!(
            "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : \"key\"\ndeclare const key: Step<[{}]>\nexport interface X {{\n  [key]: number\n  [name: string]: number\n}}\n",
            "0,".repeat(60)
        );
        let (mut compiled, files) =
            compile_and_get_checker_and_source_files_internal(t, &[("/.src/test.ts", &source)]);
        let c = &mut compiled.checker;

        let x = api_stability_test_export(t, c, files["/.src/test.ts"], "X");
        let before = c.total_instantiation_count;
        let infos = get_declared_index_infos_of_symbol(c, Some(x));
        let delta = c.total_instantiation_count - before;
        if delta != 0 {
            t.fatal(ets_go::sprintf!(
                "declared index accessor instantiated %d types, want 0",
                i64::from(delta)
            ));
        }
        let globals = c.get_global_diagnostics();
        if !globals.is_empty() {
            t.fatal(format!(
                "declared index accessor produced global diagnostics: {} diagnostics",
                globals.len()
            ));
        }
        if infos.len() != 1 {
            t.fatal(ets_go::sprintf!("got %d declared index infos, want 1", infos.len() as i64));
        }
        if infos[0].value_type().is_some() {
            t.fatal(format!("unresolved index value type should stay nil: {:?}", infos[0]));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:103 TestApiStabilityDeclaredCacheSharedAcrossTypeParsers db643a2f63294019
// TestApiStabilityDeclaredCacheSharedAcrossTypeParsers proves declared lookups
// persist per checker: a second TypeParser built from the same checker reuses
// the first parser's declared results, so a run over a different source file
// does not recompute the same tag lookup. Computed-surface persistence is
// covered by TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse.
#[test]
fn test_api_stability_declared_cache_shared_across_type_parsers() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                (
                    "/.src/dep.ts",
                    "/** @stability experimental */\nexport interface E { value: string }\nexport interface Leaky { value: E }\n",
                ),
                (
                    "/.src/consumer.ts",
                    "export interface Consumer { direct: import(\"./dep\").E }\n",
                ),
            ],
        );
        prime_api_stability_checker(t, &mut compiled);

        let dep = files["/.src/dep.ts"];
        let consumer = files["/.src/consumer.ts"];
        let leaky = api_stability_test_export(t, &mut compiled.checker, dep, "Leaky");

        let mut tp1 = compiled.type_parser();
        if tp1.links().api_stability_declared_symbol.contains_key(&leaky) {
            t.fatal("declared cache should start empty");
        }
        let deps = tp1.api_stability_used_by_symbol(leaky);
        if !contains_dependency(&deps, "E") {
            t.fatal(format!("Leaky should expose E, got {}", names(&deps)));
        }
        if !tp1.links().api_stability_declared_symbol.contains_key(&leaky) {
            t.fatal("a declared read during the analysis should be cached");
        }
        // Rust: the parsers cannot both borrow the checker; tp1's links outlive it.
        let links1 = Arc::clone(&tp1.links);
        drop(tp1);

        // A fresh parser over the same checker (the situation when another source
        // file later runs the rule) shares the declared EffectLinks cache.
        let mut tp2 = compiled.type_parser();
        if !Arc::ptr_eq(&tp2.links, &links1) {
            t.fatal("TypeParser instances over one checker should share EffectLinks");
        }
        if !tp2.links().api_stability_declared_symbol.contains_key(&leaky) {
            t.fatal("second parser did not see the cached declared symbol");
        }
        let got = tp2.api_stability_used_by_symbol(leaky);
        if !contains_dependency(&got, "E") {
            t.fatal(format!("second parser returned a different surface: {}", names(&got)));
        }

        // A symbol declared in a different source file resolves through the same
        // declared store, which is what lets separate file runs reuse tag lookups.
        let consumer_sym = api_stability_test_export(t, tp2.checker, consumer, "Consumer");
        let consumer_deps = tp2.api_stability_used_by_symbol(consumer_sym);
        if !contains_dependency(&consumer_deps, "E") {
            t.fatal(format!("Consumer should expose E directly, got {}", names(&consumer_deps)));
        }
        if !links1.lock().api_stability_declared_symbol.contains_key(&consumer_sym) {
            t.fatal("first parser did not see the cross-file cached declared symbol");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:161 TestApiStabilityNestedNamedSurfaceStaysShallow e291a863313bfcb3
// TestApiStabilityNestedNamedSurfaceStaysShallow proves the documented shallow
// named-dependency boundary: a named interface reached inside another surface is
// reported as a dependency but its arbitrary members are not expanded into the
// owner's dependencies. Directly exposed callable signatures are still searched.
#[test]
fn test_api_stability_nested_named_surface_stays_shallow() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\nexport interface E { value: string }\nexport interface Hidden { value: E }\ninterface Callable { (x: E): void }\nexport declare const exposed: { hidden: Hidden }\nexport interface CallableHolder { callable: Callable }\n",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);

        let test_file = files["/.src/test.ts"];
        let mut tp = compiled.type_parser();

        let hidden = api_stability_test_export(t, tp.checker, test_file, "Hidden");
        if !contains_dependency(&tp.api_stability_used_by_symbol(hidden), "E") {
            t.fatal("Hidden's own surface should expose E");
        }

        let exposed = api_stability_test_export(t, tp.checker, test_file, "exposed");
        let exposed_deps = tp.api_stability_used_by_symbol(exposed);
        if contains_dependency(&exposed_deps, "E") {
            t.fatal(format!("nested named Hidden must stay shallow, got {}", names(&exposed_deps)));
        }

        let holder = api_stability_test_export(t, tp.checker, test_file, "CallableHolder");
        let holder_deps = tp.api_stability_used_by_symbol(holder);
        if !contains_dependency(&holder_deps, "E") {
            t.fatal(format!(
                "directly exposed public call signatures should expose E, got {}",
                names(&holder_deps)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:203 TestApiStabilityCacheDeclaredCeilingIsPerAlias 0b3099603b9199a9
// TestApiStabilityCacheDeclaredCeilingIsPerAlias verifies the declared-stability
// cache stores the symbol's own declaration tag only. A tag on a selected-file
// `export ... from` declaration is source-specific and is applied by the rule,
// so it must not leak into the reusable symbol cache. Both aliases resolve to
// the same target, whose dependency surface is one shared per-checker result
// keyed by the concrete target identity, independent of the alias and of any
// forwarding ceiling.
#[test]
fn test_api_stability_cache_declared_ceiling_is_per_alias() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                (
                    "/.src/dep.ts",
                    "/** @stability experimental */\nexport interface E { value: string }\nexport interface Leaky { value: E }\n",
                ),
                ("/.src/plain.ts", "export { Leaky } from \"./dep\"\n"),
                (
                    "/.src/tagged.ts",
                    "/** @stability experimental */\nexport { Leaky } from \"./dep\"\n",
                ),
            ],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let plain = api_stability_test_export(t, tp.checker, files["/.src/plain.ts"], "Leaky");
        let tagged = api_stability_test_export(t, tp.checker, files["/.src/tagged.ts"], "Leaky");

        // The symbol-level lookup sees only the forwarded symbol's own declarations,
        // so the selected-file forwarding tag is not part of the shared cache.
        let level = tp.declared_api_stability(plain);
        if level != ApiStabilityLevel::Stable {
            t.fatal(format!("plain reexport symbol level = {}, want stable", level as i64));
        }
        let level = tp.declared_api_stability(tagged);
        if level != ApiStabilityLevel::Stable {
            t.fatal(format!(
                "selected-file forwarding tag leaked into the symbol cache: {}",
                level as i64
            ));
        }

        // Both aliases resolve to the same target, so the actual surface (which
        // contains E) is computed independently of any forwarding ceiling.
        let target = resolve_aliased_symbol_for_test(t, tp.checker, Some(plain));
        if !contains_dependency(&tp.api_stability_used_by_symbol(target), "E") {
            t.fatal("target surface should expose E");
        }
        if !tp.links().api_stability_declared_symbol.contains_key(&target) {
            t.fatal("the declared target lookup should be cached");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:243 resolveAliasedSymbolForTest 9a1e621bc0689ad8
pub(crate) fn resolve_aliased_symbol_for_test(
    t: &T,
    c: &mut Checker,
    symbol: Option<P<Symbol>>,
) -> P<Symbol> {
    t.helper();
    let mut seen: FxHashSet<P<Symbol>> = FxHashSet::default();
    let mut symbol = symbol;
    while let Some(current) = symbol
        && current.flags().intersects(SymbolFlags::Alias)
    {
        if seen.contains(&current) {
            break;
        }
        seen.insert(current);
        let next = c.get_immediate_aliased_symbol_exported(current);
        if next == Some(current) {
            break;
        }
        symbol = next;
    }
    let Some(symbol) = symbol else {
        t.fatal("alias resolution failed");
    };
    symbol
}

// go test: internal/typeparser/api_stability_test.go:266 TestApiStabilityCacheDifferentCheckerIsolation 27235a48a984a805
// TestApiStabilityCacheDifferentCheckerIsolation proves two checkers do not
// share declared results or pointer identity and that each analyzer computes an
// independent session-local surface.
#[test]
fn test_api_stability_cache_different_checker_isolation() {
    run(|t| {
        t.parallel();

        let source = "/** @stability experimental */\ninterface E { value: string }\nexport interface Leaky { value: E }\n";

        let (mut compiled1, sf1) = compile_and_get_checker_and_source_file_internal(t, source);
        let (mut compiled2, sf2) = compile_and_get_checker_and_source_file_internal(t, source);
        prime_api_stability_checker(t, &mut compiled1);
        prime_api_stability_checker(t, &mut compiled2);
        let mut tp1 = compiled1.type_parser();
        let mut tp2 = compiled2.type_parser();

        if Arc::ptr_eq(&tp1.links, &tp2.links) {
            t.fatal("different checkers must not share EffectLinks");
        }

        let sym1 = api_stability_test_export(t, tp1.checker, sf1, "Leaky");
        let sym2 = api_stability_test_export(t, tp2.checker, sf2, "Leaky");
        if sym1 == sym2 {
            t.fatal("checkers should produce distinct symbol pointers");
        }

        if !contains_dependency(&tp1.api_stability_used_by_symbol(sym1), "E") {
            t.fatal("first checker surface should expose E");
        }
        if !tp1.links().api_stability_declared_symbol.contains_key(&sym1) {
            t.fatal("first checker should have cached its declared lookup");
        }
        if tp2.links().api_stability_declared_symbol.contains_key(&sym1) {
            t.fatal("second checker observed the first checker's declared cache");
        }
        if !contains_dependency(&tp2.api_stability_used_by_symbol(sym2), "E") {
            t.fatal("second checker surface should expose E independently");
        }
        if tp1.links().api_stability_declared_symbol.contains_key(&sym2) {
            t.fatal("first checker observed the second checker's declared cache");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:311 TestApiStabilityCacheGenericArgumentsDoNotContaminate 81fa642068ddcea8
// TestApiStabilityCacheGenericArgumentsDoNotContaminate proves the type-keyed
// cache keeps distinct instantiations distinct: an experimental type argument
// in one instantiation must not appear in another, in both declaration orders.
#[test]
fn test_api_stability_cache_generic_arguments_do_not_contaminate() {
    run(|t| {
        t.parallel();

        for order in ["leaky-first", "clean-first"] {
            t.run(order, |t| {
                t.parallel();
                let leaky_declaration = "export type Leaky = Box<E>";
                let clean_declaration = "export type Clean = Box<string>";
                let mut declarations = format!("{leaky_declaration}\n{clean_declaration}");
                if order == "clean-first" {
                    declarations = format!("{clean_declaration}\n{leaky_declaration}");
                }

                let source = format!(
                    "/** @stability experimental */\ninterface E {{ value: string }}\ninterface Box<T> {{ value: T }}\n{declarations}\n"
                );
                let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                    t,
                    &[("/.src/test.ts", &source)],
                );
                prime_api_stability_checker(t, &mut compiled);
                let mut tp = compiled.type_parser();

                let leaky = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "Leaky");
                let clean = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "Clean");

                if !contains_dependency(&tp.api_stability_used_by_symbol(leaky), "E") {
                    t.fatal("Box<E> should expose E");
                }
                if contains_dependency(&tp.api_stability_used_by_symbol(clean), "E") {
                    let used = tp.api_stability_used_by_symbol(clean);
                    t.fatal(format!("Box<string> must not inherit E, got {}", names(&used)));
                }
            });
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:349 TestApiStabilityDeclaredReadsDoNotCompute 8edf9d12e0ab2f24
// TestApiStabilityDeclaredReadsDoNotCompute proves the declared caches are
// independent of the computed caches: asking for a declared level must never
// trigger a symbol or signature surface computation.
#[test]
fn test_api_stability_declared_reads_do_not_compute() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\nexport interface Leaky { value: E }\nexport function f(x: E): E { return x }\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let leaky = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "Leaky");
        let f = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "f");
        let signatures = api_stability_test_signatures(t, tp.checker, Some(f));

        let before = tp.checker.total_instantiation_count;
        let level = tp.declared_api_stability(leaky);
        if level != ApiStabilityLevel::Stable {
            t.fatal(format!("Leaky declared = {}, want stable", level as i64));
        }
        let _ = tp.declared_api_stability_of_signature(signatures[0]);
        let delta = tp.checker.total_instantiation_count - before;
        if delta != 0 {
            t.fatal(ets_go::sprintf!(
                "declared reads instantiated %d types, want 0",
                i64::from(delta)
            ));
        }

        // A fresh computed session starts empty: declared reads never populate a
        // symbol or signature memo and never publish a shared surface.
        let session = tp.new_api_stability_session();
        if !session.analysis.session_symbols.is_empty()
            || !session.analysis.session_types.is_empty()
            || !session.analysis.session_signatures.is_empty()
            || !session.analysis.session_components.is_empty()
        {
            t.fatal("declared reads left computed analysis state");
        }
        if !tp.links().api_stability_declared_symbol.contains_key(&leaky) {
            t.fatal("declared symbol should be cached");
        }
        let raw = raw_signature(Some(signatures[0])).expect("raw signature");
        if !tp.links().api_stability_declared_signature.contains_key(&raw) {
            t.fatal("declared signature should be cached");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:392 TestApiStabilitySignatureDeclaredCacheKeepsOverloadsDistinct d88bd400812f8f76
// TestApiStabilitySignatureDeclaredCacheKeepsOverloadsDistinct proves a tagged
// overload keeps its own declared level even when the symbol-level lookup picks
// up another declaration's tag, and that both are cached separately.
#[test]
fn test_api_stability_signature_declared_cache_keeps_overloads_distinct() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "export function f(x: string): string\n/** @stability unstable */\nexport function f(x: number): string\nexport function f(x: string | number): string { return \"\" }\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let f = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "f");
        let signatures = api_stability_test_signatures(t, tp.checker, Some(f));
        if signatures.len() != 2 {
            t.fatal(ets_go::sprintf!(
                "expected two public overloads, got %d",
                signatures.len() as i64
            ));
        }

        let first = tp.declared_api_stability_of_signature(signatures[0]);
        let second = tp.declared_api_stability_of_signature(signatures[1]);
        if first.level == second.level {
            t.fatal(format!(
                "distinct overload tags collapsed: {} and {}",
                first.level as i64, second.level as i64
            ));
        }
        if first.level != ApiStabilityLevel::Stable || second.level != ApiStabilityLevel::Unstable {
            t.fatal(format!(
                "unexpected overload levels: first={} second={}",
                first.level as i64, second.level as i64
            ));
        }
        let raw0 = raw_signature(Some(signatures[0])).expect("raw signature");
        let raw1 = raw_signature(Some(signatures[1])).expect("raw signature");
        if !tp.links().api_stability_declared_signature.contains_key(&raw0)
            || !tp.links().api_stability_declared_signature.contains_key(&raw1)
        {
            t.fatal("both overload declarations should be cached separately");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:426 apiStabilityTestSignatures 0166fbdc0fa6dbd5
pub(crate) fn api_stability_test_signatures(
    t: &T,
    c: &mut Checker,
    symbol: Option<P<Symbol>>,
) -> Vec<P<Signature>> {
    t.helper();
    let Some(symbol) = symbol else {
        t.fatal("nil symbol");
    };
    let symbol_type = c.get_type_of_symbol_exported(symbol);
    let signatures = c.get_signatures_of_type_exported(symbol_type, SignatureKind::Call).to_vec();
    if signatures.is_empty() {
        t.fatal(ets_go::sprintf!("symbol %q has no call signatures", symbol.name()));
    }
    signatures
}

// go test: internal/typeparser/api_stability_test.go:441 TestApiStabilitySignatureSurfaceRetainsSubstitution 2fe9058bb965db48
// TestApiStabilitySignatureSurfaceRetainsSubstitution proves an instantiated
// inferred return keeps its represented argument while the raw target stays
// shallow, and that a session memoizes the completed signature surface.
#[test]
fn test_api_stability_signature_surface_retains_substitution() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\ndeclare function make<T>(): (x: T) => T\nexport const f = make<E>()\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let f = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "f");
        let signatures = api_stability_test_signatures(t, tp.checker, Some(f));
        let instantiated = signatures[0];

        let mut session = tp.new_api_stability_session();
        let computed = session.used_by_signature(&mut tp, instantiated);
        if !contains_dependency(&computed, "E") {
            t.fatal(format!("instantiated signature should expose E, got {}", names(&computed)));
        }
        let key = ApiStabilitySignatureKey { signature: instantiated, carrier: None };
        if !session.analysis.session_signatures.get(&key).is_some_and(|surface| surface.complete) {
            t.fatal("completed signature surface should be memoized in the session");
        }
        let work = session.analysis.work;
        let repeat = session.used_by_signature(&mut tp, instantiated);
        if !contains_dependency(&repeat, "E") {
            t.fatal(format!("repeated session query lost E, got {}", names(&repeat)));
        }
        if session.analysis.work != work {
            t.fatal("repeated session query recomputed the memoized signature surface");
        }

        // The raw target's type parameter is not an experimental dependency, so the
        // shallow form must not report E.
        let raw = raw_signature(Some(instantiated)).expect("raw signature");
        if raw == instantiated {
            t.fatal("expected an instantiated signature with a raw target");
        }
        let shallow = tp.api_stability_used_by_signature(raw);
        if contains_dependency(&shallow, "E") {
            t.fatal(format!("raw target should stay shallow, got {}", names(&shallow)));
        }
    });
}

/// Rust-only: the body shared by the substitution-context tests: compiles `prelude` plus the `clean`/`leaky`
/// declarations in `order` and checks the clean root stays clean and the leaky one exposes E (Go repeats it).
fn check_clean_and_leaky_roots(
    t: &T,
    order: &str,
    prelude: &str,
    clean: &str,
    leaky: &str,
    clean_message: &str,
    leaky_message: &str,
) {
    let mut declarations = format!("{clean}\n{leaky}");
    if order == "leaky-first" {
        declarations = format!("{leaky}\n{clean}");
    }
    let source = format!("{prelude}{declarations}\n");
    let (mut compiled, files) =
        compile_and_get_checker_and_source_files_internal(t, &[("/.src/test.ts", &source)]);
    prime_api_stability_checker(t, &mut compiled);
    let mut tp = compiled.type_parser();

    let clean_sym = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "clean");
    let leaky_sym = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "leaky");

    let clean_used = tp.api_stability_used_by_symbol(clean_sym);
    if contains_dependency(&clean_used, "E") {
        t.fatal(format!("{clean_message}, got {}", names(&clean_used)));
    }
    let leaky_used = tp.api_stability_used_by_symbol(leaky_sym);
    if !contains_dependency(&leaky_used, "E") {
        t.fatal(format!("{leaky_message}, got {}", names(&leaky_used)));
    }
}

// go test: internal/typeparser/api_stability_test.go:490 TestApiStabilityComputedSignatureCacheSeparatesSubstitutionContexts 05a4600d758cb2f7
// TestApiStabilityComputedSignatureCacheSeparatesSubstitutionContexts proves the
// same raw signature analyzed under different enclosing substitutions is not
// conflated: the clean instantiation stays clean and the leaking one reports,
// regardless of which is computed first.
#[test]
fn test_api_stability_computed_signature_cache_separates_substitution_contexts() {
    run(|t| {
        t.parallel();

        for order in ["clean-first", "leaky-first"] {
            t.run(order, |t| {
                t.parallel();
                check_clean_and_leaky_roots(
                    t,
                    order,
                    "/** @stability experimental */\ninterface E { value: string }\ndeclare function make<T>(): (x: T) => T\n",
                    "export const clean = make<string>()",
                    "export const leaky = make<E>()",
                    "clean instantiation must not inherit E",
                    "leaky instantiation should expose E",
                );
            });
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:531 TestApiStabilityComputedSignatureCacheSeparatesNestedSubstitutionContexts 754c75e8a375fd33
// TestApiStabilityComputedSignatureCacheSeparatesNestedSubstitutionContexts
// covers the same contamination through a nested inferred object surface. The
// computed signature cache is keyed by the concrete signature identity, so the
// clean and leaking materializations never share a cached result in either
// order.
#[test]
fn test_api_stability_computed_signature_cache_separates_nested_substitution_contexts() {
    run(|t| {
        t.parallel();

        for order in ["clean-first", "leaky-first"] {
            t.run(order, |t| {
                t.parallel();
                check_clean_and_leaky_roots(
                    t,
                    order,
                    "/** @stability experimental */\ninterface E { value: string }\ndeclare function make<T>(): { nested: { fn: (x: T) => T } }\n",
                    "export const clean = make<string>()",
                    "export const leaky = make<E>()",
                    "clean nested instantiation must not inherit E",
                    "leaky nested instantiation should expose E",
                );
            });
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:570 TestApiStabilityComputedIncludesDeclaredMemberStability 7e521e569495e34e
// TestApiStabilityComputedIncludesDeclaredMemberStability proves the computed
// minimum covers the declared stability of public properties and call
// signatures, with provenance naming the offender.
#[test]
fn test_api_stability_computed_includes_declared_member_stability() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "export interface O {\n  /** @stability experimental */\n  value: string\n}\nexport interface S {\n  /** @stability unstable */\n  (x: string): string\n}\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let o = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "O");
        let o_used = tp.api_stability_used_by_symbol(o);
        if o_used.minimum != ApiStabilityLevel::Experimental {
            t.fatal(format!(
                "O minimum = {}, want experimental: {}",
                o_used.minimum as i64,
                names(&o_used)
            ));
        }
        if !contains_dependency(&o_used, "value") {
            t.fatal(format!("O should expose its tagged property, got {}", names(&o_used)));
        }

        let s = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "S");
        let s_used = tp.api_stability_used_by_symbol(s);
        if s_used.minimum != ApiStabilityLevel::Unstable {
            t.fatal(format!(
                "S minimum = {}, want unstable: {}",
                s_used.minimum as i64,
                names(&s_used)
            ));
        }
        let mut found_signature = false;
        for dependency in &s_used.dependencies {
            if dependency.signature.is_some()
                && dependency.level == ApiStabilityLevel::Unstable
                && dependency.declaration.is_some()
            {
                found_signature = true;
            }
        }
        if !found_signature {
            t.fatal(format!(
                "S should expose its tagged call signature with provenance, got {}",
                names(&s_used)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:614 TestApiStabilityReplacedDefaultStaysClean 380f263a45b3d213
// TestApiStabilityReplacedDefaultStaysClean proves a generic default replaced by
// an explicit argument is not part of the represented surface, including the
// annotation provenance pass.
#[test]
fn test_api_stability_replaced_default_stays_clean() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\ndeclare function identity<T = E>(x: T): T\nexport const f = identity<string>\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let f = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "f");
        let used = tp.api_stability_used_by_symbol(f);
        if contains_dependency(&used, "E") {
            t.fatal(format!("replaced default must stay clean, got {}", names(&used)));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:635 TestApiStabilitySignatureCacheIsolation 3621dd6a51076c42
// TestApiStabilitySignatureCacheIsolation proves a computed signature surface
// is checker-local: two checkers compute the same-looking signature
// independently and neither can observe the other's analysis.
#[test]
fn test_api_stability_signature_cache_isolation() {
    run(|t| {
        t.parallel();

        let source = "/** @stability experimental */\ninterface E { value: string }\nexport const f = (x: E): E => x\n";

        let (mut compiled1, sf1) = compile_and_get_checker_and_source_file_internal(t, source);
        let (mut compiled2, sf2) = compile_and_get_checker_and_source_file_internal(t, source);
        prime_api_stability_checker(t, &mut compiled1);
        prime_api_stability_checker(t, &mut compiled2);
        let mut tp1 = compiled1.type_parser();
        let mut tp2 = compiled2.type_parser();

        if Arc::ptr_eq(&tp1.links, &tp2.links) {
            t.fatal("different checkers must not share EffectLinks");
        }

        let f1 = api_stability_test_export(t, tp1.checker, sf1, "f");
        let f2 = api_stability_test_export(t, tp2.checker, sf2, "f");
        let sig1 = api_stability_test_signatures(t, tp1.checker, Some(f1))[0];
        let sig2 = api_stability_test_signatures(t, tp2.checker, Some(f2))[0];

        if !contains_dependency(&tp1.api_stability_used_by_signature(sig1), "E") {
            t.fatal("first checker signature should expose E");
        }
        if !contains_dependency(&tp2.api_stability_used_by_signature(sig2), "E") {
            t.fatal("second checker signature should expose E independently");
        }
        // A fresh session never observes an earlier session's local signature
        // memo, whichever checker or signature produced an earlier result; only a
        // published complete snapshot can be reused, and it is composed anew.
        let session = tp2.new_api_stability_session();
        if !session.analysis.session_signatures.is_empty() {
            t.fatal("a fresh session observed an earlier session's local signature memo");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:676 TestApiStabilityCacheCyclesAndEmptyResultsAreSafe 03aaae3fcc7b4356
// TestApiStabilityCacheCyclesAndEmptyResultsAreSafe proves a cyclic surface
// still reports its leak, that settled results are reusable inside the session
// that computed them, and that a stable/empty surface is a complete memo entry.
#[test]
fn test_api_stability_cache_cycles_and_empty_results_are_safe() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\nexport interface Node { next: Node; payload: E }\ntype B = { c: C; payload: E }\ntype C = { b: B }\nexport type A = { b: B }\nexport type Z = { c: C }\nexport interface Stable { value: string }\n",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let test_file = files["/.src/test.ts"];
        let node = api_stability_test_export(t, tp.checker, test_file, "Node");
        let mut session = tp.new_api_stability_session();
        if !contains_dependency(&session.used_by_symbol(&mut tp, node), "E") {
            t.fatal("self-referential Node should expose E");
        }
        // A settled cyclic surface is complete and reusable within its session.
        let surface = session.analysis.session_symbols.get(&node).cloned().unwrap_or_default();
        if !surface.complete || surface.blocked {
            t.fatal("a settled cyclic surface should be a complete session result");
        }
        let work = session.analysis.work;
        if !contains_dependency(&session.used_by_symbol(&mut tp, node), "E") {
            t.fatal("repeated session query lost Node's E");
        }
        if session.analysis.work != work {
            t.fatal("repeated session query recomputed the settled cyclic surface");
        }
        // The same symbol computation starts fresh for a new export: symbol results
        // stay analysis-local, and only complete type/signature snapshots may be
        // reused by a later session.
        if !tp.new_api_stability_session().analysis.session_symbols.is_empty() {
            t.fatal("a fresh session observed the earlier computed symbol surface");
        }

        // A leak reached through a mutual cycle must be reported for every root,
        // regardless of which root was analyzed first.
        let a = api_stability_test_export(t, tp.checker, test_file, "A");
        let z = api_stability_test_export(t, tp.checker, test_file, "Z");
        if !contains_dependency(&tp.api_stability_used_by_symbol(a), "E") {
            t.fatal("A should expose E through the B/C cycle");
        }
        if !contains_dependency(&tp.api_stability_used_by_symbol(z), "E") {
            t.fatal("Z should expose E through the B/C cycle");
        }
        if !tp.new_api_stability_session().analysis.session_symbols.is_empty() {
            t.fatal("mutual-cycle queries published symbol state");
        }

        let stable = api_stability_test_export(t, tp.checker, test_file, "Stable");
        let mut stable_session = tp.new_api_stability_session();
        let stable_used = stable_session.used_by_symbol(&mut tp, stable);
        if !stable_used.dependencies.is_empty() {
            t.fatal(format!("Stable should have no dependencies, got {}", names(&stable_used)));
        }
        let surface =
            stable_session.analysis.session_symbols.get(&stable).cloned().unwrap_or_default();
        if !surface.complete || surface.blocked {
            t.fatal("stable/empty results should be complete session entries");
        }
    });
}

/// Rust-only: the body shared by the concrete-carrier tests: compiles `prelude` plus the `clean`/`leaky`
/// declarations in `order` and checks the exports named `clean_name`/`leaky_name` (Go repeats it).
fn check_concrete_carriers(
    t: &T,
    order: &str,
    prelude: &str,
    (clean, clean_name, clean_message): (&str, &str, &str),
    (leaky, leaky_name, leaky_message): (&str, &str, &str),
) {
    let mut declarations = format!("{clean}\n{leaky}");
    if order == "leaky-first" {
        declarations = format!("{leaky}\n{clean}");
    }
    let source = format!("{prelude}{declarations}\n");
    let (mut compiled, files) =
        compile_and_get_checker_and_source_files_internal(t, &[("/.src/test.ts", &source)]);
    prime_api_stability_checker(t, &mut compiled);
    let mut tp = compiled.type_parser();

    let clean_sym = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], clean_name);
    let leaky_sym = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], leaky_name);
    if contains_dependency(&tp.api_stability_used_by_symbol(clean_sym), "E") {
        t.fatal(clean_message);
    }
    if !contains_dependency(&tp.api_stability_used_by_symbol(leaky_sym), "E") {
        t.fatal(leaky_message);
    }
}

// go test: internal/typeparser/api_stability_test.go:747 TestApiStabilityConcreteCarrierSeparatesSharedRawTargets ef2b09bf69f29c54
// TestApiStabilityConcreteCarrierSeparatesSharedRawTargets proves the type and
// signature caches keep concrete identities: the same raw component (an
// interface member whose type depends on the interface's type parameter) is
// analyzed under two distinct concrete outer materializations and must not
// contaminate either direction, in both declaration orders.
#[test]
fn test_api_stability_concrete_carrier_separates_shared_raw_targets() {
    run(|t| {
        t.parallel();

        for order in ["clean-first", "leaky-first"] {
            t.run(order, |t| {
                t.parallel();
                check_concrete_carriers(
                    t,
                    order,
                    "/** @stability experimental */\ninterface E { value: string }\ntype Deep<T> = T extends string ? Deep<T> : T\ninterface DeepHolder<T> { value: Deep<T> }\n",
                    (
                        "export declare const clean: DeepHolder<string>",
                        "clean",
                        "DeepHolder<string> must not expose E",
                    ),
                    (
                        "export declare const leaky: DeepHolder<E>",
                        "leaky",
                        "DeepHolder<E> should expose E",
                    ),
                );
            });
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:784 TestApiStabilityConcreteCarrierSeparatesHeritageMaterializations 8b94eeb221b82ed6
// TestApiStabilityConcreteCarrierSeparatesHeritageMaterializations covers the
// same concrete-identity requirement through inherited members: two exports
// inherit the same shared base declaration with different concrete arguments.
#[test]
fn test_api_stability_concrete_carrier_separates_heritage_materializations() {
    run(|t| {
        t.parallel();

        for order in ["clean-first", "leaky-first"] {
            t.run(order, |t| {
                t.parallel();
                check_concrete_carriers(
                    t,
                    order,
                    "/** @stability experimental */\ninterface E { value: string }\ninterface Base<T> { value: T }\n",
                    (
                        "export interface Clean extends Base<string> {}",
                        "Clean",
                        "Clean extends Base<string> and must not expose E",
                    ),
                    (
                        "export interface Leaky extends Base<E> {}",
                        "Leaky",
                        "Leaky extends Base<E> and should expose E",
                    ),
                );
            });
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:819 TestApiStabilityCycleOrderDoesNotHideLeaks c138388df0363fb7
// TestApiStabilityCycleOrderDoesNotHideLeaks analyzes the mutual cycle roots in
// the reverse order so a cached intermediate result can never suppress the leak.
#[test]
fn test_api_stability_cycle_order_does_not_hide_leaks() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\ntype B = { c: C; payload: E }\ntype C = { b: B }\nexport type A = { b: B }\nexport type Z = { c: C }\n",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let test_file = files["/.src/test.ts"];
        let z = api_stability_test_export(t, tp.checker, test_file, "Z");
        if !contains_dependency(&tp.api_stability_used_by_symbol(z), "E") {
            t.fatal("Z analyzed first should still expose E through the cycle");
        }
        let a = api_stability_test_export(t, tp.checker, test_file, "A");
        if !contains_dependency(&tp.api_stability_used_by_symbol(a), "E") {
            t.fatal("A analyzed second should still expose E through the cached cycle");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:850 TestApiStabilityColdLazyReadsMaterialize 6fb146f59bac78f9
// TestApiStabilityColdLazyReadsMaterialize proves a cold public surface is
// materialized through the checker's ordinary lazy reads: the analysis no
// longer needs the language service to warm the checker first. The cold surface
// exposes E and is complete in its session, and the same checker after type
// checking recomputes to the same surface from a fresh session.
#[test]
fn test_api_stability_cold_lazy_reads_materialize() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\ntype Deep<T> = T extends string ? Deep<T> : T\ninterface Callable<T> { (x: T): void }\nexport declare const f: Callable<Deep<E>>\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let f = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "f");
        let mut cold_session = tp.new_api_stability_session();
        let cold = cold_session.used_by_symbol(&mut tp, f);
        if !contains_dependency(&cold, "E") {
            t.fatal(format!("the cold surface should lazily materialize E, got {}", names(&cold)));
        }
        let surface = cold_session.analysis.session_symbols.get(&f).cloned().unwrap_or_default();
        if !surface.complete || surface.blocked {
            t.fatal("a complete cold surface should settle complete in its session");
        }
        drop(tp);

        // A warmed checker materializes the declaration the way the language
        // service would, and a fresh session agrees with the cold result.
        prime_api_stability_checker(t, &mut compiled);
        let mut fresh = compiled.type_parser();
        let warm = fresh.api_stability_used_by_symbol(f);
        if !contains_dependency(&warm, "E") {
            t.fatal("the materialized surface should expose E");
        }
        if cold.minimum != warm.minimum || cold.dependencies.len() != warm.dependencies.len() {
            t.fatal(format!(
                "cold and warm results disagree: cold={} warm={}",
                names(&cold),
                names(&warm)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:890 TestApiStabilityColdLazyFiniteApplicationMatchesWarm c419bf7f4e117ba4
// TestApiStabilityColdLazyFiniteApplicationMatchesWarm proves a finite
// recursive alias application resolves through the ordinary lazy read on a
// genuinely cold checker, and the cold session result matches the materialized
// recomputation in a new session.
#[test]
fn test_api_stability_cold_lazy_finite_application_matches_warm() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\ntype A<T> = T extends string ? A<number> : E\nexport declare const x: A<string>\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let x = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "x");
        let cold = tp.api_stability_used_by_symbol(x);
        if !contains_dependency(&cold, "E") {
            t.fatal(format!("the cold surface should lazily resolve E, got {}", names(&cold)));
        }
        drop(tp);

        prime_api_stability_checker(t, &mut compiled);
        let warm = compiled.type_parser().api_stability_used_by_symbol(x);
        if !contains_dependency(&warm, "E") {
            t.fatal(format!("the materialized surface should expose E, got {}", names(&warm)));
        }

        // The materialized checker is recomputed by a fresh session; it must agree
        // with the first cold and warm results.
        let recomputed = compiled.type_parser().api_stability_used_by_symbol(x);
        if !contains_dependency(&recomputed, "E") {
            t.fatal(format!(
                "the materialized recomputation should expose E, got {}",
                names(&recomputed)
            ));
        }
        if recomputed.minimum != warm.minimum
            || recomputed.dependencies.len() != warm.dependencies.len()
        {
            t.fatal(format!(
                "warm results disagree: first={} recomputed={}",
                names(&warm),
                names(&recomputed)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:932 TestApiStabilityExplicitStableDeclaredPresence 7adf8d6eabdaca61
// TestApiStabilityExplicitStableDeclaredPresence pins that branch-style
// `@stability stable` tags are parsed as an explicit value, not as an absent
// tag. The root symbol and each public signature keep their declaration
// provenance in the per-checker declared caches, while an untagged overload
// stays stable with no declaration. The leak rule's ceiling and the forwarding
// presence check read these caches, so explicit stable roots and explicitly
// stable signatures stay distinguishable from untagged ones.
#[test]
fn test_api_stability_explicit_stable_declared_presence() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability stable */\nexport function tagged(x: string): string\nexport function tagged(x: number): number\nexport function untagged(x: string): string\nexport function untagged(x: number): number\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let tagged = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "tagged");
        let untagged = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "untagged");

        let tagged_declared = tp.declared_api_stability_of_symbol(tagged);
        if tagged_declared.level != ApiStabilityLevel::Stable
            || tagged_declared.declaration.is_none()
        {
            t.fatal(format!("explicit stable root lost its provenance: {tagged_declared:?}"));
        }
        if !tp.links().api_stability_declared_symbol.contains_key(&tagged) {
            t.fatal("explicit stable root should be cached");
        }
        let declared = tp.declared_api_stability_of_symbol(untagged);
        if declared.level != ApiStabilityLevel::Stable || declared.declaration.is_some() {
            t.fatal(format!("untagged root should be stable without provenance: {declared:?}"));
        }

        let tagged_signatures = api_stability_test_signatures(t, tp.checker, Some(tagged));
        let untagged_signatures = api_stability_test_signatures(t, tp.checker, Some(untagged));
        if tagged_signatures.len() != 2 || untagged_signatures.len() != 2 {
            t.fatal(ets_go::sprintf!(
                "unexpected overload counts: tagged=%d untagged=%d",
                tagged_signatures.len() as i64,
                untagged_signatures.len() as i64
            ));
        }
        for (index, &signature) in tagged_signatures.iter().enumerate() {
            let declared = tp.declared_api_stability_of_signature(signature);
            let raw = raw_signature(Some(signature)).expect("raw signature");
            if !tp.links().api_stability_declared_signature.contains_key(&raw) {
                t.fatal(ets_go::sprintf!("tagged overload %d should be cached", index as i64));
            }
            if index == 0 {
                if declared.level != ApiStabilityLevel::Stable || declared.declaration.is_none() {
                    t.fatal(format!("explicit stable overload lost its provenance: {declared:?}"));
                }
                let declaration = declared.declaration.expect("checked above");
                if !tp.links().api_stability_declared_declaration.contains_key(&declaration) {
                    t.fatal("explicit stable declaration should be cached");
                }
            } else if declared.level != ApiStabilityLevel::Stable || declared.declaration.is_some()
            {
                t.fatal(format!(
                    "untagged overload should be stable without provenance: {declared:?}"
                ));
            }
        }
        for (index, &signature) in untagged_signatures.iter().enumerate() {
            let declared = tp.declared_api_stability_of_signature(signature);
            if declared.declaration.is_some() {
                t.fatal(format!("untagged signature {index} has a declaration: {declared:?}"));
            }
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:994 TestApiStabilityComputedPublicMembers 719cdb6eaab82f8e
// TestApiStabilityComputedPublicMembers pins that a root public surface
// traverses public computed members of interfaces, classes and anonymous object
// types: well-known symbols, custom unique symbols and literal computed names
// all contribute their represented member types, while private and protected
// computed members stay excluded. It also pins that a checker late-bound name
// never leaks into a diagnostic: a tagged computed member renders its readable
// declaration spelling.
#[test]
fn test_api_stability_computed_public_members() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r#"/** @stability experimental */
interface E { x: string }
/** @stability experimental */
type P = string
const custom = Symbol()
const privateKey = Symbol()
export interface OnlyComputed { [Symbol.iterator](): E; [custom]: P }
export interface MixedComputed {
  plain: number
  [Symbol.iterator](): E
  [custom]: P
  ["literal"]: P
}
export interface TaggedComputed {
  /** @stability experimental */
  [Symbol.iterator](): string
  /** @stability experimental */
  [custom]: string
}
export class ComputedClass {
  plain: number
  [Symbol.iterator](): E
  [custom]: P
  private [privateKey]: E
  protected [Symbol.toStringTag](): E
  static [custom]: P
  static [Symbol.iterator](): E
}
export class HiddenComputed {
  private [privateKey]: E
  protected [Symbol.toStringTag](): E
}
export declare const computedObject: {
  [Symbol.iterator](): E
  [custom]: P
  ["literal"]: P
}
"#,
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        for name in ["OnlyComputed", "MixedComputed", "ComputedClass", "computedObject"] {
            let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], name);
            let mut session = tp.new_api_stability_session();
            let used = session.used_by_symbol(&mut tp, symbol);
            let dependency_names = names(&used);
            if !contains_dependency(&used, "E") {
                t.error(format!(
                    "{name} lost its computed public dependency E: {dependency_names}"
                ));
            }
            if !contains_dependency(&used, "P") {
                t.error(format!(
                    "{name} lost its computed public dependency P: {dependency_names}"
                ));
            }
            let surface =
                session.analysis.session_symbols.get(&symbol).cloned().unwrap_or_default();
            if !surface.complete || surface.blocked {
                t.error(format!(
                    "{name} computed member surface should settle complete: {surface:?}"
                ));
            }
        }

        let hidden =
            api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "HiddenComputed");
        let used = tp.api_stability_used_by_symbol(hidden);
        if contains_dependency(&used, "E") {
            t.error(format!(
                "private/protected computed members leaked into the surface: {}",
                names(&used)
            ));
        }

        let tagged =
            api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "TaggedComputed");
        let used = tp.api_stability_used_by_symbol(tagged);
        let mut rendered: FxHashSet<String> = FxHashSet::default();
        for dependency in &used.dependencies {
            rendered.insert(api_stability_dependency_name(dependency));
            // Rust: Go's `"\xfe"` is tsrs's internal-name prefix.
            if api_stability_dependency_name(dependency).starts_with(INTERNAL_SYMBOL_NAME_PREFIX) {
                t.error(format!(
                    "late-bound internal name leaked into a diagnostic: {:?}",
                    api_stability_dependency_name(dependency)
                ));
            }
        }
        if !rendered.contains("[Symbol.iterator]") || !rendered.contains("[custom]") {
            t.error(format!(
                "tagged computed members should render readable names, got {}",
                names(&used)
            ));
        }
        if used.dependencies.len() < 2 {
            t.error(format!(
                "tagged computed members should report separately, got {}",
                names(&used)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1084 TestApiStabilityEarlyTableComputedMembersNotCachedComplete f0c4ec53afb2568a
// TestApiStabilityEarlyTableComputedMembersNotCachedComplete pins the
// complete-result invariant for the early member table: an interface whose
// computed member has not been late-bound yet must stay incomplete and never be
// reported as a complete surface, because its early table omits the member.
// Once the checker materializes the member table a retry settles complete with
// the computed dependency.
#[test]
fn test_api_stability_early_table_computed_members_not_cached_complete() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { x: string }\nexport interface WellKnownOnly { plain: number; [Symbol.iterator](): E }\nexport class ColdClass { plain: number; static [Symbol.iterator](): E }\n",
            )],
        );
        let mut tp = compiled.type_parser();
        for name in ["WellKnownOnly", "ColdClass"] {
            let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], name);
            let declared = tp.checker.get_declared_type_of_symbol_exported(symbol);
            if get_resolved_members_of_type_if_materialized(Some(declared)).is_some() {
                t.fatal(format!(
                    "{name}: expected an unmaterialized member table for the early-table path"
                ));
            }
            let mut session = tp.new_api_stability_session();
            let used = session.used_by_symbol(&mut tp, symbol);
            if contains_dependency(&used, "E") {
                t.fatal(format!(
                    "{name}: the early table cannot see a late-bound member; the surface must stay incomplete"
                ));
            }
            let surface =
                session.analysis.session_symbols.get(&symbol).cloned().unwrap_or_default();
            if surface.complete {
                t.fatal(format!(
                    "{name}: an incomplete surface must never settle complete: {surface:?}"
                ));
            }
        }
        drop(tp);
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();
        for name in ["WellKnownOnly", "ColdClass"] {
            let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], name);
            let declared = tp.checker.get_declared_type_of_symbol_exported(symbol);
            if get_resolved_members_of_type_if_materialized(Some(declared)).is_none() {
                t.fatal(format!(
                    "{name}: expected the member table to be materialized after checking"
                ));
            }
            let mut session = tp.new_api_stability_session();
            let used = session.used_by_symbol(&mut tp, symbol);
            if !contains_dependency(&used, "E") {
                t.fatal(format!(
                    "{name}: retry after materialization lost the computed dependency: {}",
                    names(&used)
                ));
            }
            let surface =
                session.analysis.session_symbols.get(&symbol).cloned().unwrap_or_default();
            if !surface.complete {
                t.fatal(format!("{name}: retry should settle a complete surface: {surface:?}"));
            }
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1132 TestApiStabilitySettleSkipRequiresNoAdvancement 9f9aae44969f6ab2
// TestApiStabilitySettleSkipRequiresNoAdvancement pins the corrected
// settlement predicate: a blocked surface is only skipped after the guard
// budget is exhausted when no analysis read advanced the materialization epoch
// since it was collected and no earlier settle round advanced any surface.
#[test]
fn test_api_stability_settle_skip_requires_no_advancement() {
    run(|t| {
        t.parallel();

        let (_compiled, _files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[("/.src/test.ts", "export interface Box<A> { value: A }")],
        );
        let mut analysis = new_api_stability_analysis();
        let mut surface = new_api_stability_surface();
        surface.block();

        if analysis.settle_cannot_advance(&surface) {
            t.fatal("a surface must not be skipped while the guard budget is available");
        }
        analysis.safety_work = API_STABILITY_SAFETY_MAX_WORK + 1;
        surface.observed_epoch = 7;
        analysis.materialization_epoch = 7;
        if !analysis.settle_cannot_advance(&surface) {
            t.fatal("a blocked surface with no advancement since collection must be skipped");
        }
        analysis.materialization_epoch = 8;
        if analysis.settle_cannot_advance(&surface) {
            t.fatal("a materialization advancement must force a recollection");
        }
        analysis.materialization_epoch = 7;
        let mut complete = new_api_stability_surface();
        complete.observed_epoch = 7;
        if analysis.settle_cannot_advance(&complete) {
            t.fatal("only blocked surfaces are skipped");
        }
    });
}

/// Rust-only: the cold `Root`/`fn` fixture the settlement callback tests share (Go repeats it): reads the root's
/// declared type and its members, and checks the `a` member and the `z` function's inferred return are still cold.
fn cold_root_members(t: &T, c: &mut Checker, sym: P<Symbol>) -> (P<Symbol>, Vec<P<Signature>>) {
    let _ = c.get_declared_type_of_symbol_exported(sym);
    let value = symbol_table_get(sym.members(), "a").expect("member a");
    let z = symbol_table_get(sym.members(), "z").expect("member z");
    let function_type = c.get_type_of_symbol_exported(z);
    let signatures = c.get_signatures_of_type_exported(function_type, SignatureKind::Call).to_vec();
    if signatures.len() != 1 {
        t.fatal("signature missing");
    }
    if get_resolved_return_type_of_signature_if_materialized(Some(signatures[0])).is_some() {
        t.fatal("expected cold inferred return");
    }
    if get_resolved_type_of_symbol_if_materialized(c, Some(value)).is_some() {
        t.fatal("expected cold a member");
    }
    (value, signatures)
}

// go test: internal/typeparser/api_stability_test.go:1170 TestApiStabilitySettleRecollectsMaterializedMember f7c20332595c312d
// TestApiStabilitySettleRecollectsMaterializedMember pins the
// P1 within one analysis: a component the guard refused while the budget was
// exhausted can be materialized by a later ordinary read in the same collection,
// and settlement must recollect the blocked root instead of assuming it cannot
// advance. The result still stays blocked so a later analysis retries it,
// and no new guard read is consumed.
#[test]
fn test_api_stability_settle_recollects_materialized_member() {
    run(|t| {
        t.parallel();
        // Rust-only: the after-check callback is one process-wide slot (see `lock_after_check_callback`).
        let _guard = ets_go::testing::lock_after_check_callback();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                (
                    "/.src/dep.ts",
                    "/** @stability experimental */\ninterface E { value: string }\nexport interface Root { a: E; z: typeof fn }\ndeclare const object: Root\nfunction fn() { return object.a.value.length }",
                ),
                ("/.src/test.ts", "export { Root } from \"./dep.js\""),
            ],
        );
        let sym =
            api_stability_test_export(t, &mut compiled.checker, files["/.src/dep.ts"], "Root");
        let (value, _signatures) = cold_root_members(t, &mut compiled.checker, sym);
        let ran = Arc::new(AtomicBool::new(false));
        let checker_id = compiled.checker.id;
        let program = compiled.program;
        let test_file = files["/.src/test.ts"];
        let callback_t = t.clone();
        let callback_ran = Arc::clone(&ran);
        register_after_check_source_file_callback(Arc::new(move |_ctx, _program, cc, sf| {
            if cc.id != checker_id || sf != test_file {
                return;
            }
            let t = &callback_t;
            callback_ran.store(true, Ordering::SeqCst);
            let mut tp = new_type_parser(program, cc);
            let mut analysis = new_api_stability_analysis();
            analysis.safety_work = API_STABILITY_SAFETY_MAX_WORK + 1;
            analysis.symbol_surface(&mut tp, Some(sym));
            if get_resolved_type_of_symbol_if_materialized(tp.checker, Some(value)).is_none() {
                t.error("later ordinary read should have materialized the refused member");
            }
            analysis.settle(&mut tp);
            let surface = analysis.session_symbols.get(&sym).cloned().unwrap_or_default();
            let used = api_stability_used_of(&surface, Some(sym));
            if !contains_dependency(&used, "E") {
                t.error(format!(
                    "settle did not retry a newly materialized member: {}",
                    names(&used)
                ));
            }
            if analysis.safety_work != API_STABILITY_SAFETY_MAX_WORK + 1 {
                t.error(format!(
                    "post-exhaustion recurrence consumed guard budget: {}",
                    analysis.safety_work
                ));
            }
            if !surface.blocked {
                t.fatal("a recollection that stays unavailable must remain blocked");
            }
        }));
        let _ = compiled.checker.get_diagnostics_exported(&Context::background(), test_file);
        if !ran.load(Ordering::SeqCst) {
            t.fatal("callback did not run");
        }

        // A later analysis on the materialized checker retries and settles complete.
        let mut tp = compiled.type_parser();
        let mut fresh = new_api_stability_analysis();
        fresh.symbol_surface(&mut tp, Some(sym));
        fresh.settle(&mut tp);
        let surface = fresh.session_symbols.get(&sym).cloned().unwrap_or_default();
        let used = api_stability_used_of(&surface, Some(sym));
        if !contains_dependency(&used, "E") {
            t.fatal(format!("retry analysis lost the dependency: {}", names(&used)));
        }
        if !surface.complete {
            t.fatal(format!("retry analysis should settle a complete surface: {surface:?}"));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1244 TestApiStabilityNaturalBudgetMaterializationRetry f9c9ffbaf0e0474d
// TestApiStabilityNaturalBudgetMaterializationRetry reproduces the natural-budget
// probe: the shared guard budget is exhausted by an
// ordinary member while a later inferred return materializes a previously
// refused experimental member. Settlement must collect the finding, keep the
// side effects bounded and leave the surface blocked for a later retry.
#[test]
fn test_api_stability_natural_budget_materialization_retry() {
    run(|t| {
        t.parallel();
        // Rust-only: the after-check callback is one process-wide slot (see `lock_after_check_callback`).
        let _guard = ets_go::testing::lock_after_check_callback();

        let dep = format!(
            "type Heavy = [{}]\n/** @stability experimental */\ninterface E {{ value: string }}\nexport interface Root {{ \"0heavy\": Heavy; a: E; z: typeof fn }}\ndeclare const object: Root\nfunction fn() {{ return object.a.value.length }}",
            "string,".repeat(17000)
        );
        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[("/.src/dep.ts", &dep), ("/.src/test.ts", "export { Root } from \"./dep.js\"")],
        );
        let sym =
            api_stability_test_export(t, &mut compiled.checker, files["/.src/dep.ts"], "Root");
        let _ = cold_root_members(t, &mut compiled.checker, sym);
        let ran = Arc::new(AtomicBool::new(false));
        let checker_id = compiled.checker.id;
        let program = compiled.program;
        let test_file = files["/.src/test.ts"];
        let callback_t = t.clone();
        let callback_ran = Arc::clone(&ran);
        register_after_check_source_file_callback(Arc::new(move |_ctx, _program, cc, sf| {
            if cc.id != checker_id || sf != test_file {
                return;
            }
            let t = &callback_t;
            callback_ran.store(true, Ordering::SeqCst);
            let mut tp = new_type_parser(program, cc);
            let mut analysis = new_api_stability_analysis();
            analysis.symbol_surface(&mut tp, Some(sym));
            if !analysis.safety_budget_exhausted() {
                t.fatal("the heavy member should exhaust the shared guard budget");
            }
            analysis.settle(&mut tp);
            let surface = analysis.session_symbols.get(&sym).cloned().unwrap_or_default();
            let used = api_stability_used_of(&surface, Some(sym));
            if !contains_dependency(&used, "E") {
                t.error(format!(
                    "natural-budget settle missed the materialized member: {}",
                    names(&used)
                ));
            }
            if !surface.blocked {
                t.fatal("the root remains blocked and must not settle stable");
            }
        }));
        let _ = compiled.checker.get_diagnostics_exported(&Context::background(), test_file);
        if !ran.load(Ordering::SeqCst) {
            t.fatal("callback did not run");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1311 TestApiStabilityRecoveryPropagatesFindingsToEarlierResults b746df13c60bf11f
// TestApiStabilityRecoveryPropagatesFindingsToEarlierResults pins the
// recovery settlement defect: a recovery round can discover a
// finding in a component after the fixpoint has converged, but the blocked
// parent that was collected before that discovery must be revisited within the
// same bounded recovery loop. Advancing the materialization epoch after a
// changed recovery round makes exactly those earlier results stale, so the root
// reflects the finding its own component discovered even though the analysis
// stays exhausted and the root remains blocked and never settled as stable.
//
// Without the advancement the component holds `E` after the recovery sweep
// while the root still reports nothing: the export's returned Used set (what
// the rule reports) misses the dependency, and the caller sees a stable
// surface that a later analysis would have reported. The test consumes no
// additional guard work: recovery recollects represented components through
// ordinary reads, it never authorizes a new guarded read.
#[test]
fn test_api_stability_recovery_propagates_findings_to_earlier_results() {
    run(|t| {
        t.parallel();
        // Rust-only: the after-check callback is one process-wide slot (see `lock_after_check_callback`).
        let _guard = ets_go::testing::lock_after_check_callback();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                (
                    "/.src/dep.ts",
                    "/** @stability experimental */\ninterface E { value: string }\ndeclare function make(): { a: E; z: typeof fn }\nexport const root = make()\nfunction fn() { return root.a.value.length }",
                ),
                ("/.src/test.ts", "export { root } from \"./dep.js\""),
            ],
        );

        let c = &mut compiled.checker;
        let sym = api_stability_test_export(t, c, files["/.src/dep.ts"], "root");
        let typ = c.get_type_of_symbol_exported(sym);
        let members = c.get_properties_of_type_exported(typ);
        let (mut value, mut fn_symbol) = (None, None);
        for &m in members {
            if m.name() == "a" {
                value = Some(m);
            }
            if m.name() == "z" {
                fn_symbol = Some(m);
            }
        }
        let (Some(value), Some(fn_symbol)) = (value, fn_symbol) else {
            t.fatal("expected members a and z");
        };
        let fn_type = c.get_type_of_symbol_exported(fn_symbol);
        let signature = c.get_signatures_of_type_exported(fn_type, SignatureKind::Call)[0];
        if get_resolved_return_type_of_signature_if_materialized(Some(signature)).is_some()
            || get_resolved_type_of_symbol_if_materialized(c, Some(value)).is_some()
        {
            t.fatal("expected cold member and inferred return");
        }

        let ran = Arc::new(AtomicBool::new(false));
        let checker_id = compiled.checker.id;
        let program = compiled.program;
        let test_file = files["/.src/test.ts"];
        let callback_t = t.clone();
        let callback_ran = Arc::clone(&ran);
        register_after_check_source_file_callback(Arc::new(move |_ctx, _program, cc, sf| {
            if cc.id != checker_id || sf != test_file {
                return;
            }
            let t = &callback_t;
            callback_ran.store(true, Ordering::SeqCst);
            let mut tp = new_type_parser(program, cc);
            let mut analysis = new_api_stability_analysis();
            analysis.safety_work = API_STABILITY_SAFETY_MAX_WORK + 1;
            // A cycle-cut signature contributes no findings until it is recollected,
            // which is what makes the recovery sweep the only round that can find it.
            // The result is stored directly, exactly like an independent seeded
            // analysis; settlement must reconcile it into its visit order.
            analysis
                .session_signatures
                .insert(ApiStabilitySignatureKey { signature, carrier: None }, cycle_cut_surface());
            analysis.symbol_surface(&mut tp, Some(sym));
            if get_resolved_type_of_symbol_if_materialized(tp.checker, Some(value)).is_some() {
                t.fatal("member was materialized before settlement");
            }
            analysis.settle(&mut tp);
            if analysis.safety_work != API_STABILITY_SAFETY_MAX_WORK + 1 {
                t.error(format!("recovery consumed guard work: {}", analysis.safety_work));
            }
            let surface = analysis.session_symbols.get(&sym).cloned().unwrap_or_default();
            if !surface.blocked {
                t.error("the exhausted root must stay blocked, not settle stable");
            }
            let used = api_stability_used_of(&surface, Some(sym));
            let mut found = false;
            for component in analysis.session_components.values() {
                if contains_dependency(&api_stability_used_of(component, None), "E") {
                    found = true;
                }
            }
            if !found {
                t.fatal(format!(
                    "recovery sweep did not recollect the component: {}",
                    names(&used)
                ));
            }
            if !contains_dependency(&used, "E") {
                t.error(format!(
                    "recovery found E in a component but failed to propagate it to the root: {}",
                    names(&used)
                ));
            }
        }));
        let _ = compiled.checker.get_diagnostics_exported(&Context::background(), test_file);
        if !ran.load(Ordering::SeqCst) {
            t.fatal("callback did not run");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1399 TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse 2d2de59710f12a30
// TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse pins the
// current cache policy (updated from the original local-only policy,
// which the latest shared-surface design supersedes): declared symbol/signature
// stability persists per checker and is shared across analyses and TypeParser
// instances; a complete, settled, context-free concrete type or signature
// surface is published to the per-checker shared caches and reused by later
// exports, files and parsers; symbol results and every incomplete or
// substitution-context result stay local to the session that computed them.
// Declared lookups never trigger a computed analysis.
#[test]
fn test_api_stability_declared_persists_and_complete_surfaces_reuse() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/dep.ts",
                "/** @stability experimental */\nexport interface E { value: string }\nexport interface Leaky { value: E }\nexport interface AlsoLeaky { other: E }\n",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let leaky = api_stability_test_export(t, tp.checker, files["/.src/dep.ts"], "Leaky");
        let also_leaky =
            api_stability_test_export(t, tp.checker, files["/.src/dep.ts"], "AlsoLeaky");

        // A fresh session starts with no local computed state: symbol results and
        // unpublishable results are never carried by an earlier session.
        let mut session = tp.new_api_stability_session();
        if !session.analysis.session_symbols.is_empty()
            || !session.analysis.session_types.is_empty()
            || !session.analysis.session_signatures.is_empty()
            || !session.analysis.session_components.is_empty()
        {
            t.fatal("a fresh session must start with no local computed state");
        }

        let used = session.used_by_symbol(&mut tp, leaky);
        if used.incomplete || !contains_dependency(&used, "E") {
            t.fatal(format!(
                "Leaky should be a complete surface exposing E, got {} (incomplete={})",
                names(&used),
                used.incomplete
            ));
        }

        // Repeating the same component inside the export reuses the session memo:
        // the second query consumes no analysis work and returns the same surface.
        let work = session.analysis.work;
        let repeat = session.used_by_symbol(&mut tp, leaky);
        if repeat.incomplete || !contains_dependency(&repeat, "E") {
            t.fatal(format!(
                "repeated session query lost E, got {} (incomplete={})",
                names(&repeat),
                repeat.incomplete
            ));
        }
        if session.analysis.work != work {
            t.fatal("repeated session query recomputed a completed surface");
        }

        // The complete concrete type surface was published: a new export still
        // starts with an empty symbol memo, yet composes the same result and a
        // direct type query consumes no analysis work at all.
        let declared = tp.checker.get_declared_type_of_symbol_exported(leaky);
        if api_stability_shared_type_entry(&tp, Some(declared), ApiStabilityInspection::Expand)
            .is_none()
        {
            t.fatal("the complete Leaky type surface should be published for reuse");
        }
        let mut next = tp.new_api_stability_session();
        if !next.analysis.session_symbols.is_empty() {
            t.fatal("a new export must not observe an earlier export's symbol memo");
        }
        let next_used = next.used_by_symbol(&mut tp, also_leaky);
        if next_used.incomplete || !contains_dependency(&next_used, "E") {
            t.fatal(format!(
                "AlsoLeaky should be a complete surface exposing E, got {} (incomplete={})",
                names(&next_used),
                next_used.incomplete
            ));
        }
        let mut direct = new_api_stability_analysis();
        direct.type_surface(
            &mut tp,
            Some(declared),
            ApiStabilityInspection::Expand,
            &ApiStabilitySubstitution::default(),
        );
        if direct.work != 0 {
            t.fatal(format!("the published type snapshot consumed {} work", direct.work));
        }
        drop(tp);

        // Declared stability persists per checker and is shared across parsers;
        // the declared read never triggers a computed analysis or a publication.
        let mut tp2 = compiled.type_parser();
        if !tp2.links().api_stability_declared_symbol.contains_key(&leaky) {
            t.fatal("declared symbol should be cached per checker");
        }
        let tag_only = api_stability_test_export(t, tp2.checker, files["/.src/dep.ts"], "E");
        let tag_only_type = tp2.checker.get_declared_type_of_symbol_exported(tag_only);
        if api_stability_shared_type_entry(
            &tp2,
            Some(tag_only_type),
            ApiStabilityInspection::Expand,
        )
        .is_some()
        {
            t.fatal("the tagged E type must not be published before a computed query");
        }
        let before = tp2.checker.total_instantiation_count;
        let declared_level = tp2.declared_api_stability(leaky);
        if declared_level != ApiStabilityLevel::Stable {
            t.fatal(format!("Leaky declared = {}, want stable", declared_level as i64));
        }
        let delta = tp2.checker.total_instantiation_count - before;
        if delta != 0 {
            t.fatal(ets_go::sprintf!(
                "declared read instantiated %d types, want 0",
                i64::from(delta)
            ));
        }
        drop(tp2);

        // Reused snapshots still produce the same surface content.
        let mut fresh_tp = compiled.type_parser();
        let mut fresh = fresh_tp.new_api_stability_session();
        let again = fresh.used_by_symbol(&mut fresh_tp, leaky);
        if again.incomplete || again.dependencies.len() != used.dependencies.len() {
            t.fatal(format!(
                "a later session disagrees with the first: first={} again={}",
                names(&used),
                names(&again)
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1492 TestApiStabilityIncompleteIsNotStable 5cb0a37b70cca1d7
// TestApiStabilityIncompleteIsNotStable pins that a surface the checker
// has not materialized reports Incomplete from the public result, publishes
// nothing, and that a later session over the materialized checker recomputes a
// complete result: an incomplete result is never carried over as a finished
// surface.
#[test]
fn test_api_stability_incomplete_is_not_stable() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\nexport interface WellKnownOnly { plain: number; [Symbol.iterator](): E }\n",
            )],
        );
        let mut tp = compiled.type_parser();

        let symbol =
            api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "WellKnownOnly");
        let declared = tp.checker.get_declared_type_of_symbol_exported(symbol);
        let cold = tp.api_stability_used_by_symbol(symbol);
        if !cold.incomplete {
            t.fatal("an unmaterialized computed member must report an incomplete result");
        }
        if api_stability_shared_type_entry(&tp, Some(declared), ApiStabilityInspection::Expand)
            .is_some()
        {
            t.fatal("an incomplete result must never be published");
        }
        drop(tp);

        prime_api_stability_checker(t, &mut compiled);
        let mut warm_tp = compiled.type_parser();
        let warm = warm_tp.api_stability_used_by_symbol(symbol);
        if warm.incomplete || !contains_dependency(&warm, "E") {
            t.fatal(format!(
                "the materialized retry should settle complete with E, got {} (incomplete={})",
                names(&warm),
                warm.incomplete
            ));
        }
        if api_stability_shared_type_entry(&warm_tp, Some(declared), ApiStabilityInspection::Expand)
            .is_none()
        {
            t.fatal("the materialized retry should publish its complete surface");
        }
    });
}
