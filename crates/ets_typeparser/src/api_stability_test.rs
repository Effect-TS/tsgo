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

use crate::api_stability_surface_test::{
    api_stability_shared_signature_entry, api_stability_shared_type_entry,
    assert_api_stability_surface_excludes_own_tag,
};
use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/api_stability_test.go:15 apiStabilityTestExport f28b1fb5adf53da6
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

// go test: internal/typeparser/api_stability_test.go:47 primeApiStabilityChecker d22211443a6d7d32
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

// go test: internal/typeparser/api_stability_test.go:56 TestApiStabilityDeclaredIndexInfosDoNotResolveComputedNames 40ae3567958cd774
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

// go test: internal/typeparser/api_stability_test.go:88 TestApiStabilityDeclaredCacheSharedAcrossTypeParsers 7a3cc3ac5b1e24bb
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

// go test: internal/typeparser/api_stability_test.go:138 TestApiStabilityNestedNamedSurfaceStaysShallow 8db6898117729933
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

// go test: internal/typeparser/api_stability_test.go:173 TestApiStabilityCacheDeclaredCeilingIsPerAlias 81504979902a3b09
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

        let target = resolve_aliased_symbol_for_test(t, tp.checker, Some(plain));
        if !contains_dependency(&tp.api_stability_used_by_symbol(target), "E") {
            t.fatal("target surface should expose E");
        }
        if !tp.links().api_stability_declared_symbol.contains_key(&target) {
            t.fatal("the declared target lookup should be cached");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:209 resolveAliasedSymbolForTest 9a1e621bc0689ad8
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

// go test: internal/typeparser/api_stability_test.go:229 TestApiStabilitySignatureDeclaredCacheKeepsOverloadsDistinct 3ee91d5f43dfe447
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

// go test: internal/typeparser/api_stability_test.go:261 apiStabilityTestSignatures 5d9d0853c8093485
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

// go test: internal/typeparser/api_stability_test.go:273 TestApiStabilitySignatureSurfaceRetainsSubstitution ab7a1be642390022
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

// go test: internal/typeparser/api_stability_test.go:316 TestApiStabilityComputedIncludesDeclaredMemberStability 9b67130bc4422959
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

// go test: internal/typeparser/api_stability_test.go:357 TestApiStabilityReplacedDefaultStaysClean b64bd67afc390667
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

// go test: internal/typeparser/api_stability_test.go:375 TestApiStabilityCacheCyclesAndEmptyResultsAreSafe 99cb8309652861a6
#[test]
fn test_api_stability_cache_cycles_and_empty_results_are_safe() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */\ninterface E { value: string }\n/** @stability unstable */\nexport interface Node { next: Node; payload: E }\nexport interface Stable { value: string }\n",
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let test_file = files["/.src/test.ts"];
        let node = api_stability_test_export(t, tp.checker, test_file, "Node");
        let mut session = tp.new_api_stability_session();
        let node_used = session.used_by_symbol(&mut tp, node);
        if node_used.incomplete
            || node_used.minimum != ApiStabilityLevel::Experimental
            || !contains_dependency(&node_used, "E")
        {
            t.fatal(format!(
                "self-referential Node should expose E, got min={} deps={}",
                node_used.minimum as i64,
                names(&node_used)
            ));
        }
        assert_api_stability_surface_excludes_own_tag(t, &node_used, "Node");
        let surface = session.analysis.session_symbols.get(&node).cloned().unwrap_or_default();
        if !surface.complete || surface.blocked {
            t.fatal("a settled cyclic surface should be a complete session result");
        }
        if session
            .analysis
            .session_symbols
            .get(&node)
            .is_none_or(|surface| surface.findings.is_empty())
        {
            t.fatal("the stored cyclic surface must keep the own tag");
        }
        let work = session.analysis.work;
        if !contains_dependency(&session.used_by_symbol(&mut tp, node), "E") {
            t.fatal("repeated session query lost Node's E");
        }
        if session.analysis.work != work {
            t.fatal("repeated session query recomputed the settled cyclic surface");
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

// go test: internal/typeparser/api_stability_test.go:422 TestApiStabilityColdLazyFiniteApplicationMatchesWarm 42f25a832b16af4c
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

// go test: internal/typeparser/api_stability_test.go:455 TestApiStabilityExplicitStableDeclaredPresence e04d48572f1c2bf0
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

// go test: internal/typeparser/api_stability_test.go:510 TestApiStabilityComputedPublicMembers feba370ada52456b
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

// go test: internal/typeparser/api_stability_test.go:594 TestApiStabilityEarlyTableComputedMembersNotCachedComplete 04a7e6a0b5015fcc
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

// go test: internal/typeparser/api_stability_test.go:638 TestApiStabilitySettleSkipRequiresNoAdvancement f639cfc42120b742
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

// go test: internal/typeparser/api_stability_test.go:670 TestApiStabilitySettleRecollectsMaterializedMember 3b6d169f5528e4eb
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

// go test: internal/typeparser/api_stability_test.go:738 TestApiStabilityNaturalBudgetMaterializationRetry 500741f636d399b0
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

// go test: internal/typeparser/api_stability_test.go:790 TestApiStabilityRecoveryPropagatesFindingsToEarlierResults f30ea94a3ef52d1c
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

// go test: internal/typeparser/api_stability_test.go:865 TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse 83d3e9af02afa291
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

        let mut session = tp.new_api_stability_session();
        let used = session.used_by_symbol(&mut tp, leaky);
        if used.incomplete || !contains_dependency(&used, "E") {
            t.fatal(format!(
                "Leaky should be a complete surface exposing E, got {} (incomplete={})",
                names(&used),
                used.incomplete
            ));
        }

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

        let declared = tp.checker.get_declared_type_of_symbol_exported(leaky);
        if api_stability_shared_type_entry(&tp, Some(declared), ApiStabilityInspection::Expand)
            .is_none()
        {
            t.fatal("the complete Leaky type surface should be published for reuse");
        }
        let mut next = tp.new_api_stability_session();
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

// go test: internal/typeparser/api_stability_test.go:935 TestApiStabilityInstantiationsDoNotContaminate 370bf817fab50e85
#[test]
fn test_api_stability_instantiations_do_not_contaminate() {
    run(|t| {
        t.parallel();

        #[derive(Clone, Copy)]
        struct Export {
            declaration: &'static str,
            name: &'static str,
            leaks: bool,
        }
        struct Case {
            name: &'static str,
            prelude: &'static str,
            exports: [Export; 2],
        }
        let export = |declaration, name, leaks| Export { declaration, name, leaks };
        let tests = [
            Case {
                name: "generic arguments",
                prelude: "interface Box<T> { value: T }",
                exports: [
                    export("export type Leaky = Box<E>", "Leaky", true),
                    export("export type Clean = Box<string>", "Clean", false),
                ],
            },
            Case {
                name: "signature substitution",
                prelude: "declare function make<T>(): (x: T) => T",
                exports: [
                    export("export const leaky = make<E>()", "leaky", true),
                    export("export const clean = make<string>()", "clean", false),
                ],
            },
            Case {
                name: "nested signature substitution",
                prelude: "declare function make<T>(): { nested: { fn: (x: T) => T } }",
                exports: [
                    export("export const leaky = make<E>()", "leaky", true),
                    export("export const clean = make<string>()", "clean", false),
                ],
            },
            Case {
                name: "shared raw member target",
                prelude: "type Deep<T> = T extends string ? Deep<T> : T\ninterface DeepHolder<T> { value: Deep<T> }",
                exports: [
                    export("export declare const leaky: DeepHolder<E>", "leaky", true),
                    export("export declare const clean: DeepHolder<string>", "clean", false),
                ],
            },
            Case {
                name: "heritage materializations",
                prelude: "interface Base<T> { value: T }",
                exports: [
                    export("export interface Leaky extends Base<E> {}", "Leaky", true),
                    export("export interface Clean extends Base<string> {}", "Clean", false),
                ],
            },
            Case {
                name: "mutual cycle roots",
                prelude: "type B = { c: C; payload: E }\ntype C = { b: B }",
                exports: [
                    export("export type A = { b: B }", "A", true),
                    export("export type Z = { c: C }", "Z", true),
                ],
            },
        ];
        for test in &tests {
            for declarations_reversed in [false, true] {
                for queries_reversed in [false, true] {
                    t.run(
                        &format!(
                            "{}/declarationsReversed={declarations_reversed}/queriesReversed={queries_reversed}",
                            test.name
                        ),
                        |t| {
                            t.parallel();
                            let mut declarations =
                                [test.exports[0].declaration, test.exports[1].declaration];
                            if declarations_reversed {
                                declarations.reverse();
                            }
                            let source = format!(
                                "/** @stability experimental */\ninterface E {{ value: string }}\n{}\n{}\n",
                                test.prelude,
                                declarations.join("\n")
                            );
                            let (mut compiled, files) =
                                compile_and_get_checker_and_source_files_internal(
                                    t,
                                    &[("/.src/test.ts", &source)],
                                );
                            prime_api_stability_checker(t, &mut compiled);
                            let mut tp = compiled.type_parser();

                            let mut exports = test.exports.to_vec();
                            if queries_reversed {
                                exports.reverse();
                            }
                            for export in exports {
                                let symbol = api_stability_test_export(
                                    t,
                                    tp.checker,
                                    files["/.src/test.ts"],
                                    export.name,
                                );
                                let used = tp.api_stability_used_by_symbol(symbol);
                                if contains_dependency(&used, "E") != export.leaks {
                                    t.fatal(format!(
                                        "{} exposes E = {}, want {}: {}",
                                        export.name,
                                        !export.leaks,
                                        export.leaks,
                                        names(&used)
                                    ));
                                }
                            }
                        },
                    );
                }
            }
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1010 TestApiStabilityDeclaredReadsDoNotCompute ca0b49d575f1f49a
#[test]
fn test_api_stability_declared_reads_do_not_compute() {
    run(|t| {
        t.parallel();

        for prime in [false, true] {
            t.run(&format!("prime={prime}"), |t| {
                t.parallel();
                let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                    t,
                    &[(
                        "/.src/test.ts",
                        "/** @stability experimental */\ninterface E { value: string }\nexport interface Leaky { value: E }\n/** @stability unstable */\nexport interface Tagged { leak: string }\n/** @stability unstable */\nexport declare function taggedFn(): string\n",
                    )],
                );
                if prime {
                    prime_api_stability_checker(t, &mut compiled);
                }
                let mut tp = compiled.type_parser();

                let test_file = files["/.src/test.ts"];
                let leaky = api_stability_test_export(t, tp.checker, test_file, "Leaky");
                let tagged = api_stability_test_export(t, tp.checker, test_file, "Tagged");
                let tagged_fn = api_stability_test_export(t, tp.checker, test_file, "taggedFn");
                let signature = api_stability_test_signatures(t, tp.checker, Some(tagged_fn))[0];

                let before = tp.checker.total_instantiation_count;
                let levels = [
                    tp.declared_api_stability(leaky),
                    tp.declared_api_stability_of_symbol(tagged).level,
                    tp.declared_api_stability_of_symbol(tagged_fn).level,
                    tp.declared_api_stability_of_signature(signature).level,
                ];
                let delta = tp.checker.total_instantiation_count - before;
                if delta != 0 {
                    t.fatal(ets_go::sprintf!(
                        "declared reads instantiated %d types, want 0",
                        i64::from(delta)
                    ));
                }
                let want = [
                    ApiStabilityLevel::Stable,
                    ApiStabilityLevel::Unstable,
                    ApiStabilityLevel::Unstable,
                    ApiStabilityLevel::Unstable,
                ];
                if levels != want {
                    t.fatal(format!(
                        "declared levels = {:?}, want {:?}",
                        levels.map(|level| level as i64),
                        want.map(|level| level as i64)
                    ));
                }
                for symbol in [leaky, tagged, tagged_fn] {
                    if !tp.links().api_stability_declared_symbol.contains_key(&symbol) {
                        t.fatal(format!("declared lookup of {} should be cached", symbol.name()));
                    }
                }
                let raw = raw_signature(Some(signature)).expect("raw signature");
                if !tp.links().api_stability_declared_signature.contains_key(&raw) {
                    t.fatal("declared signature lookup should be cached");
                }
                for symbol in [leaky, tagged] {
                    let declared = tp.checker.get_declared_type_of_symbol_exported(symbol);
                    if api_stability_shared_type_entry(
                        &tp,
                        Some(declared),
                        ApiStabilityInspection::Expand,
                    )
                    .is_some()
                    {
                        t.fatal(format!(
                            "a declared read published the {} type surface",
                            symbol.name()
                        ));
                    }
                }
                if api_stability_shared_signature_entry(&tp, Some(signature)).is_some() {
                    t.fatal("a declared read published a signature surface");
                }
                if prime {
                    let used = tp.new_api_stability_session().used_by_symbol(&mut tp, tagged_fn);
                    if used.incomplete
                        || used.minimum != ApiStabilityLevel::Stable
                        || !used.dependencies.is_empty()
                    {
                        t.fatal(format!(
                            "taggedFn surface should be empty, got min={} deps={}",
                            used.minimum as i64,
                            names(&used)
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1077 TestApiStabilityCachesAreCheckerLocal a98e679f072a32b1
#[test]
fn test_api_stability_caches_are_checker_local() {
    run(|t| {
        t.parallel();

        let source = "/** @stability experimental */\nexport interface E { value: string }\nexport interface Leaky { value: E }\nexport const f = (x: E): E => x\n/** @stability unstable */\nexport interface Shared { leak: E }\nexport interface Holder { child: Shared }\n";
        let (mut compiled1, sf1) = compile_and_get_checker_and_source_file_internal(t, source);
        let (mut compiled2, sf2) = compile_and_get_checker_and_source_file_internal(t, source);
        prime_api_stability_checker(t, &mut compiled1);
        prime_api_stability_checker(t, &mut compiled2);
        let mut tp1 = compiled1.type_parser();
        let mut tp2 = compiled2.type_parser();

        if Arc::ptr_eq(&tp1.links, &tp2.links) {
            t.fatal("different checkers must not share EffectLinks");
        }
        let leaky1 = api_stability_test_export(t, tp1.checker, sf1, "Leaky");
        let leaky2 = api_stability_test_export(t, tp2.checker, sf2, "Leaky");
        let shared1 = api_stability_test_export(t, tp1.checker, sf1, "Shared");
        let shared1 = tp1.checker.get_declared_type_of_symbol_exported(shared1);
        let shared2 = api_stability_test_export(t, tp2.checker, sf2, "Shared");
        let shared2 = tp2.checker.get_declared_type_of_symbol_exported(shared2);
        if leaky1 == leaky2 || shared1 == shared2 {
            t.fatal("checkers should produce distinct identities");
        }

        let shallow = ApiStabilityInspection::Shallow;
        query_caches_are_checker_local(t, &mut tp1, sf1);
        if !tp1.links().api_stability_declared_symbol.contains_key(&leaky1) {
            t.fatal("the first checker should cache its declared lookup");
        }
        if api_stability_shared_type_entry(&tp1, Some(shared1), shallow).is_none() {
            t.fatal("the first checker should publish its shallow snapshot");
        }
        if tp2.links().api_stability_declared_symbol.contains_key(&leaky1)
            || api_stability_shared_type_entry(&tp2, Some(shared1), shallow).is_some()
        {
            t.fatal("the second checker observed the first checker's caches");
        }
        if tp2.links().api_stability_declared_symbol.contains_key(&leaky2)
            || api_stability_shared_type_entry(&tp2, Some(shared2), shallow).is_some()
        {
            t.fatal("the second checker cached results before its own queries");
        }

        query_caches_are_checker_local(t, &mut tp2, sf2);
        if tp1.links().api_stability_declared_symbol.contains_key(&leaky2)
            || api_stability_shared_type_entry(&tp1, Some(shared2), shallow).is_some()
        {
            t.fatal("the first checker observed the second checker's caches");
        }
    });
}

// go test: internal/typeparser/api_stability_test.go:1077 TestApiStabilityCachesAreCheckerLocal a98e679f072a32b1
// Rust: Go's `query` closure.
fn query_caches_are_checker_local(t: &T, tp: &mut TypeParser<'_>, sf: P<SourceFile>) {
    t.helper();
    let leaky = api_stability_test_export(t, tp.checker, sf, "Leaky");
    if !contains_dependency(&tp.api_stability_used_by_symbol(leaky), "E") {
        t.fatal("Leaky should expose E");
    }
    let f = api_stability_test_export(t, tp.checker, sf, "f");
    let signature = api_stability_test_signatures(t, tp.checker, Some(f))[0];
    if !contains_dependency(&tp.api_stability_used_by_signature(signature), "E") {
        t.fatal("f's signature should expose E");
    }
    let holder = api_stability_test_export(t, tp.checker, sf, "Holder");
    if !contains_dependency(&tp.new_api_stability_session().used_by_symbol(tp, holder), "Shared") {
        t.fatal("Holder should expose Shared");
    }
}
