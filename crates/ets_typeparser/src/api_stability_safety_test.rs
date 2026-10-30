// Ported from effect-tsgo's internal/typeparser/api_stability_safety_test.go (see helpers_test.rs).

use ets_shim::core::context::Context;

use crate::api_stability_test::*;
use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/api_stability_safety_test.go:11 apiStabilityRecursiveAlias f2ab8274801e950f
const API_STABILITY_RECURSIVE_ALIAS: &str = "type Deep<T> = T extends string ? Deep<T> : T\n";

// go test: internal/typeparser/api_stability_safety_test.go:13 TestApiStabilitySafetyCompoundOperands 6916c9793170307c
#[test]
fn test_api_stability_safety_compound_operands() {
    run(|t| {
        t.parallel();

        let cases: [(&str, &str); 5] = [
            (
                "anonymous object compound",
                "type Select<T> = { v: T } extends { v: string } ? Deep<string> : number
export declare const x: Select<string>",
            ),
            (
                "anonymous function compound",
                "type Select<T> = (() => T) extends (() => string) ? Deep<string> : number
export declare const x: Select<string>",
            ),
            (
                "anonymous method compound",
                "type Select<T> = { m(): T } extends { m(): string } ? Deep<string> : number
export declare const x: Select<string>",
            ),
            (
                "mapped compound",
                "type Select<T> = { [K in keyof T]: Deep<K> } extends { toString(): string } ? number : Deep<string>
export declare const x: Select<string>",
            ),
            (
                "inferred class property relation",
                "declare function make<T>(): Deep<T>
class F { v = make<string>() }
type Select<T> = T extends { v: number } ? string : number
export declare const x: Select<F>",
            ),
        ];
        for (name, dep) in cases {
            t.run(name, |t| {
                t.parallel();

                let dep = format!("{API_STABILITY_RECURSIVE_ALIAS}{dep}");
                let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
                    t,
                    &[
                        ("/.src/dep.d.ts", &dep),
                        ("/.src/test.ts", r#"export { x } from "./dep.js""#),
                    ],
                );
                let mut tp = compiled.type_parser();
                let _ = tp
                    .checker
                    .get_diagnostics_exported(&Context::background(), files["/.src/test.ts"]);
                let x = api_stability_test_export(t, tp.checker, files["/.src/dep.d.ts"], "x");
                let mut analysis = new_api_stability_analysis();
                let before = tp.checker.total_instantiation_count;
                let annotation = x.declarations()[0].as_variable_declaration().type_();
                let verdict = analysis.new_safety_scan().type_node(
                    &mut tp,
                    annotation,
                    &ApiStabilitySubstitution::default(),
                    None,
                    false,
                );
                let delta = tp.checker.total_instantiation_count - before;
                if verdict == ApiStabilitySafety::Safe {
                    t.error("compound operand was authorized as safe");
                }
                if delta != 0 {
                    t.error(ets_go::sprintf!("safety scan instantiated %d types, want 0", delta));
                }
                let globals = tp.checker.get_global_diagnostics();
                if !globals.is_empty() {
                    t.error(ets_go::sprintf!(
                        "safety scan produced global diagnostics: %d",
                        globals.len() as i64
                    ));
                }
                if !analysis.session_symbols.is_empty() {
                    t.error("refused scan populated a symbol surface");
                }
            });
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:79 TestApiStabilitySafetyDeferredCompoundStaysSafe a007e0225ec67cbe
#[test]
fn test_api_stability_safety_deferred_compound_stays_safe() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "type Select<T> = { v: T } extends { v: string } ? number : T
export interface X<T> { v: Select<T> }",
            )],
        );
        let mut tp = compiled.type_parser();
        let _ = tp.checker.get_diagnostics_exported(&Context::background(), files["/.src/test.ts"]);
        let x = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "X");
        let before = tp.checker.total_instantiation_count;
        let used = tp.api_stability_used_by_symbol(x);
        let delta = tp.checker.total_instantiation_count - before;
        if !used.dependencies.is_empty() {
            t.fatal(ets_go::sprintf!(
                "deferred anonymous operand should expose nothing, got %v",
                api_stability_dependency_names(&used)
            ));
        }
        if delta != 0 {
            t.fatal(ets_go::sprintf!("deferred anonymous operand instantiated %d types", delta));
        }
        let globals = tp.checker.get_global_diagnostics();
        if !globals.is_empty() {
            t.fatal(ets_go::sprintf!(
                "deferred anonymous operand produced global diagnostics: %d",
                globals.len() as i64
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:103 TestApiStabilitySafetyStructuralConditionalStaysSelectable 5f737ca9c0f77c35
#[test]
fn test_api_stability_safety_structural_conditional_stays_selectable() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */
interface E { value: string }
interface Holder<T> { value: T extends { value: string } ? number : E }
export declare const x: Holder<{ value: string }>",
            )],
        );
        let mut tp = compiled.type_parser();
        let _ = tp.checker.get_diagnostics_exported(&Context::background(), files["/.src/test.ts"]);
        let x = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "x");
        let mut session = tp.new_api_stability_session();
        let used = session.used_by_symbol(&mut tp, x);
        if !used.dependencies.is_empty() {
            t.fatal(ets_go::sprintf!(
                "structural conditional selected the true branch; E must stay quiet: %v",
                api_stability_dependency_names(&used)
            ));
        }
        let entry = session.analysis.session_symbols.get(&x).cloned().unwrap_or_default();
        if !entry.complete || entry.blocked {
            t.fatal(ets_go::sprintf!(
                "structural conditional surface should settle complete: complete=%v blocked=%v",
                entry.complete,
                entry.blocked
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:126 TestApiStabilitySafetyTypeLiteralComputedMembers 8bf2196906312ea0
#[test]
fn test_api_stability_safety_type_literal_computed_members() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r#"/** @stability experimental */
type PropertyAlias = string
/** @stability experimental */
type ParameterAlias = string
/** @stability experimental */
type MethodAlias = string
/** @stability experimental */
type ComputedAlias = string
export declare const x: {
  [Symbol.iterator]?: PropertyAlias
  [Symbol.toStringTag](value: ParameterAlias): MethodAlias
  ["computed"]: ComputedAlias
}
"#,
            )],
        );
        let mut tp = compiled.type_parser();
        let _ = tp.checker.get_diagnostics_exported(&Context::background(), files["/.src/test.ts"]);
        let x = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "x");
        let mut session = tp.new_api_stability_session();
        let used = session.used_by_symbol(&mut tp, x);
        for want in ["PropertyAlias", "ParameterAlias", "MethodAlias", "ComputedAlias"] {
            if !contains_dependency(&used, want) {
                t.fatal(ets_go::sprintf!(
                    "computed member annotation %s lost its erased alias provenance, got %v",
                    want,
                    api_stability_dependency_names(&used)
                ));
            }
        }
        let entry = session.analysis.session_symbols.get(&x).cloned().unwrap_or_default();
        if !entry.complete || entry.blocked {
            t.fatal(ets_go::sprintf!(
                "computed member surface should settle complete: complete=%v blocked=%v",
                entry.complete,
                entry.blocked
            ));
        }
        let globals = tp.checker.get_global_diagnostics();
        if !globals.is_empty() {
            t.fatal(ets_go::sprintf!(
                "computed member traversal produced global diagnostics: %d",
                globals.len() as i64
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:164 TestApiStabilitySafetyComputedNameDisplayAndSort c1f62ce870d1f9b3
#[test]
fn test_api_stability_safety_computed_name_display_and_sort() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r#"export declare const x: {
  [Symbol.iterator]?: string
  [Symbol.toStringTag](): void
  ["computed"]: number
  [42]: boolean
}
"#,
            )],
        );
        let c = &mut *compiled.checker;
        let _ = c.get_diagnostics_exported(&Context::background(), files["/.src/test.ts"]);
        let x = api_stability_test_export(t, c, files["/.src/test.ts"], "x");
        let typ = c.get_type_of_symbol_exported(x);
        let _ = c.get_properties_of_type_exported(typ);
        let Some(members) = get_resolved_members_of_type_if_materialized(Some(typ)) else {
            t.fatal("member table should be materialized");
        };
        // Rust: tsrs spells the internal-name prefix `\u{7f}` where Go uses byte `0xFE`.
        let want_by_key: [(&str, &str); 4] = [
            ("\u{7f}@iterator@", "[Symbol.iterator]"),
            ("\u{7f}@toStringTag@", "[Symbol.toStringTag]"),
            ("computed", "computed"),
            ("42", "42"),
        ];
        let mut seen = 0;
        let entries = members.map(|table| table.entries()).unwrap_or_default();
        for (key, member) in entries {
            let mut want_rendered = "";
            for (prefix, rendered) in want_by_key {
                if key == prefix || (key.len() > prefix.len() && key.starts_with(prefix)) {
                    want_rendered = rendered;
                    break;
                }
            }
            if want_rendered.is_empty() {
                continue;
            }
            seen += 1;
            let dependency = new_api_stability_dependency(
                Some(member),
                None,
                Some(member.declarations()[0]),
                ApiStabilityLevel::Stable,
            );
            let rendered = api_stability_dependency_name(&dependency);
            if rendered != want_rendered {
                t.error(ets_go::sprintf!(
                    "member %q displayed as %q, want %q",
                    key,
                    rendered,
                    want_rendered
                ));
            }
            let sorted = api_stability_dependency_sort_name(&dependency);
            if sorted != want_rendered {
                t.error(ets_go::sprintf!(
                    "member %q sorted as %q, want %q",
                    key,
                    sorted,
                    want_rendered
                ));
            }
        }
        if seen != want_by_key.len() {
            t.fatal(ets_go::sprintf!(
                "expected %d computed members, saw %d",
                want_by_key.len() as i64,
                seen as i64
            ));
        }

        let alias = files["/.src/test.ts"].statements().nodes()[0]
            .as_variable_statement()
            .declaration_list()
            .as_variable_declaration_list()
            .declarations()
            .nodes()[0];
        let want_in_order = ["[Symbol.iterator]", "[Symbol.toStringTag]", "computed", "42"];
        let member_nodes = alias.as_variable_declaration().type_().expect("annotation").members();
        if member_nodes.len() != want_in_order.len() {
            t.fatal(ets_go::sprintf!(
                "expected %d computed members, got %d",
                want_in_order.len() as i64,
                member_nodes.len() as i64
            ));
        }
        for (index, &member) in member_nodes.iter().enumerate() {
            let dependency =
                new_api_stability_dependency(None, None, Some(member), ApiStabilityLevel::Stable);
            let rendered = api_stability_dependency_name(&dependency);
            if rendered != want_in_order[index] {
                t.error(ets_go::sprintf!(
                    "declaration %d displayed as %q, want %q",
                    index as i64,
                    rendered,
                    want_in_order[index]
                ));
            }
            let sorted = api_stability_dependency_sort_name(&dependency);
            if sorted != want_in_order[index] {
                t.error(ets_go::sprintf!(
                    "declaration %d sorted as %q, want %q",
                    index as i64,
                    sorted,
                    want_in_order[index]
                ));
            }
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:233 TestApiStabilitySafetyMaterializedReferenceSkipsGuard 14c370ae0e7a2aba
#[test]
fn test_api_stability_safety_materialized_reference_skips_guard() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                r#"export interface Box<A> { value: A }; export const box: Box<string> = { value: "x" }"#,
            )],
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();
        let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "box");
        let typ = tp.checker.get_type_of_symbol_exported(symbol);
        let _ = tp.checker.get_properties_of_type_exported(typ);
        if get_resolved_members_of_type_if_materialized(Some(typ)).is_none() {
            t.fatal("member table should be materialized");
        }
        let mut analysis = new_api_stability_analysis();
        let mut surface = new_api_stability_surface();
        analysis.collect_concrete_reference_surface(&mut tp, &mut surface, typ);
        if analysis.safety_work != 0 {
            t.fatal(ets_go::sprintf!(
                "materialized reference consumed %d guard work units, want 0",
                analysis.safety_work
            ));
        }
        if !surface.complete || surface.blocked {
            t.fatal(ets_go::sprintf!(
                "materialized reference surface should be complete, got complete=%v blocked=%v",
                surface.complete,
                surface.blocked
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:258 TestApiStabilitySafetySafeVerdictIsReused 3ade29fcadebbe45
#[test]
fn test_api_stability_safety_safe_verdict_is_reused() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "export interface Box<A> { value: A; map(fn: (value: A) => A): Box<A> }",
            )],
        );
        let mut tp = compiled.type_parser();
        let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "Box");
        // Rust: tsrs's GetDeclaredTypeOfSymbol never returns nil, so Go's "declared type missing" check drops.
        let declaration = tp.checker.get_declared_type_of_symbol_exported(symbol);
        let mut analysis = new_api_stability_analysis();
        if !analysis.member_table_resolution_is_safe(
            &mut tp,
            Some(declaration),
            &ApiStabilitySubstitution::default(),
        ) {
            t.fatal("bounded interface member table should be safe");
        }
        let spent = analysis.safety_work;
        if spent == 0 {
            t.fatal("guard unexpectedly consumed no budget on the first request");
        }
        if !analysis.member_table_resolution_is_safe(
            &mut tp,
            Some(declaration),
            &ApiStabilitySubstitution::default(),
        ) {
            t.fatal("repeated guard request must stay safe");
        }
        if analysis.safety_work != spent {
            t.fatal(ets_go::sprintf!(
                "completed Safe verdict was not reused: %d -> %d",
                spent,
                analysis.safety_work
            ));
        }
        if analysis.safety_safe_memo.is_empty() {
            t.fatal("completed Safe verdict was not recorded");
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:289 TestApiStabilitySafetyPostBudgetRefusalIsCheap 075a95337fd9e0d8
#[test]
fn test_api_stability_safety_post_budget_refusal_is_cheap() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[("/.src/test.ts", "export interface Box<A> { value: A }")],
        );
        let mut tp = compiled.type_parser();
        let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "Box");
        let declaration = tp.checker.get_declared_type_of_symbol_exported(symbol);
        let mut analysis = new_api_stability_analysis();
        analysis.safety_work = API_STABILITY_SAFETY_MAX_WORK + 1;

        if analysis.member_table_resolution_is_safe(
            &mut tp,
            Some(declaration),
            &ApiStabilitySubstitution::default(),
        ) {
            t.fatal("exhausted analysis must refuse member table resolution");
        }
        let node = declaration.symbol().expect("declared symbol").declarations()[0];
        if analysis.annotation_resolution_is_safe(
            &mut tp,
            Some(node),
            &ApiStabilitySubstitution::default(),
        ) {
            t.fatal("exhausted analysis must refuse annotation resolution");
        }
        if analysis.safety_work != API_STABILITY_SAFETY_MAX_WORK + 1 {
            t.fatal(ets_go::sprintf!(
                "post-budget refusals consumed budget: %d",
                analysis.safety_work
            ));
        }
        if !analysis.safety_safe_memo.is_empty() {
            t.fatal("post-budget refusals must not publish a Safe verdict");
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:315 TestApiStabilitySafetySettleSkipsBlockedRecollectionAfterExhaustion 859632343cc69313
#[test]
fn test_api_stability_safety_settle_skips_blocked_recollection_after_exhaustion() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[("/.src/test.ts", "export interface Box<A> { value: A }")],
        );
        let mut tp = compiled.type_parser();
        let mut analysis = new_api_stability_analysis();
        let symbol = api_stability_test_export(t, tp.checker, files["/.src/test.ts"], "Box");
        let declaration = tp.checker.get_declared_type_of_symbol_exported(symbol);
        let key = ApiStabilityTypeKey {
            t: declaration,
            inspect: ApiStabilityInspection::Expand,
            carrier: None,
        };
        let mut surface = new_api_stability_surface();
        surface.block();
        analysis.store_type_surface(key, surface);
        analysis.safety_work = API_STABILITY_SAFETY_MAX_WORK + 1;

        analysis.settle(&mut tp);
        if analysis.safety_work != API_STABILITY_SAFETY_MAX_WORK + 1 {
            t.fatal(ets_go::sprintf!(
                "settle recollection consumed budget after exhaustion: %d",
                analysis.safety_work
            ));
        }
        let entry = analysis.session_types.get(&key).cloned().unwrap_or_default();
        if entry.complete {
            t.fatal(ets_go::sprintf!(
                "blocked surface must never settle complete, got complete=%v blocked=%v",
                entry.complete,
                entry.blocked
            ));
        }
    });
}

// go test: internal/typeparser/api_stability_safety_test.go:340 TestApiStabilitySafetyTypeNameLookupCacheIsIdentitySafe 082d4934f274d6dc
#[test]
fn test_api_stability_safety_type_name_lookup_cache_is_identity_safe() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[(
                "/.src/test.ts",
                "/** @stability experimental */
interface E { value: string }
export declare function consume(x: E): E",
            )],
        );
        let mut tp = compiled.type_parser();
        let _ = tp.checker.get_diagnostics_exported(&Context::background(), files["/.src/test.ts"]);
        let mut analysis = new_api_stability_analysis();
        let declaration = files["/.src/test.ts"].statements().nodes()[1];
        let type_name = declaration.parameters()[0]
            .as_parameter_declaration()
            .type_()
            .expect("parameter annotation")
            .as_type_reference_node()
            .type_name();
        let Some(first) = analysis.symbol_at_type_name_node(&mut tp, Some(type_name)) else {
            t.fatal("type name lookup failed");
        };
        if analysis.symbol_at_type_name_node(&mut tp, Some(type_name)) != Some(first) {
            t.fatal("cached lookup changed identity");
        }
        if analysis.type_name_symbols.get(&type_name) != Some(&first) {
            t.fatal("successful lookup was not cached by AST identity");
        }
        if analysis.symbol_at_type_name_node(&mut tp, None).is_some() {
            t.fatal("nil node must not resolve");
        }
        if analysis.type_name_symbols.len() != 1 {
            t.fatal(ets_go::sprintf!(
                "failed or nil lookups must not be cached: %d entries",
                analysis.type_name_symbols.len() as i64
            ));
        }
    });
}
