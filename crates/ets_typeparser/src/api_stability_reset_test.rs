// Ported from effect-tsgo's internal/typeparser/api_stability_reset_test.go (see helpers_test.rs).

use ets_go::testing::T;
use ets_shim::ast::SymbolTable;

use crate::api_stability_surface_test::*;
use crate::api_stability_test::*;
use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/api_stability_reset_test.go:16 apiStabilityAnalysisDiff e8f80477167b4847
// apiStabilityAnalysisDiff names the fields of two analyses that differ,
// treating a nil and an empty map or slice as equal. Only one side may hold a
// non-empty collection, so it compares a used analysis against a fresh one. A
// field of a kind it does not know fails the test, so a new field forces a
// decision about reset.
//
// Rust: Go walks the fields with reflection; here both analyses are destructured without `..`, so a new field
// fails to compile until it is listed. Go's `tp` field is not stored on the Rust analysis. Names are Go's.
fn api_stability_analysis_diff(
    t: &T,
    got: &ApiStabilityAnalysis,
    want: &ApiStabilityAnalysis,
) -> Vec<&'static str> {
    t.helper();
    let ApiStabilityAnalysis {
        session_symbols,
        session_types,
        session_components,
        session_signatures,
        type_frames,
        signature_frames,
        carriers,
        settle_symbols,
        settle_types,
        settle_components,
        settle_signatures,
        scope,
        publish_types,
        publish_signatures,
        active_symbols,
        active_types,
        active_components,
        active_signatures,
        active_aliases,
        active_conditionals,
        signature_args,
        signature_parameter_cache,
        safety_safe_memo,
        type_name_symbols,
        optional_tagged_symbols,
        work,
        safety_work,
        materialization_reads,
        materialization_epoch,
    } = got;
    let ApiStabilityAnalysis {
        session_symbols: want_session_symbols,
        session_types: want_session_types,
        session_components: want_session_components,
        session_signatures: want_session_signatures,
        type_frames: want_type_frames,
        signature_frames: want_signature_frames,
        carriers: want_carriers,
        settle_symbols: want_settle_symbols,
        settle_types: want_settle_types,
        settle_components: want_settle_components,
        settle_signatures: want_settle_signatures,
        scope: want_scope,
        publish_types: want_publish_types,
        publish_signatures: want_publish_signatures,
        active_symbols: want_active_symbols,
        active_types: want_active_types,
        active_components: want_active_components,
        active_signatures: want_active_signatures,
        active_aliases: want_active_aliases,
        active_conditionals: want_active_conditionals,
        signature_args: want_signature_args,
        signature_parameter_cache: want_signature_parameter_cache,
        safety_safe_memo: want_safety_safe_memo,
        type_name_symbols: want_type_name_symbols,
        optional_tagged_symbols: want_optional_tagged_symbols,
        work: want_work,
        safety_work: want_safety_work,
        materialization_reads: want_materialization_reads,
        materialization_epoch: want_materialization_epoch,
    } = want;
    let scope_equal = api_stability_values_equal(
        t,
        "scope.symbols.keys",
        scope.symbols.keys.len(),
        want_scope.symbols.keys.len(),
    ) && api_stability_values_equal(
        t,
        "scope.symbols.has",
        scope.symbols.has.len(),
        want_scope.symbols.has.len(),
    ) && api_stability_values_equal(
        t,
        "scope.types.keys",
        scope.types.keys.len(),
        want_scope.types.keys.len(),
    ) && api_stability_values_equal(
        t,
        "scope.types.has",
        scope.types.has.len(),
        want_scope.types.has.len(),
    ) && api_stability_values_equal(
        t,
        "scope.components.keys",
        scope.components.keys.len(),
        want_scope.components.keys.len(),
    ) && api_stability_values_equal(
        t,
        "scope.components.has",
        scope.components.has.len(),
        want_scope.components.has.len(),
    ) && api_stability_values_equal(
        t,
        "scope.signatures.keys",
        scope.signatures.keys.len(),
        want_scope.signatures.keys.len(),
    ) && api_stability_values_equal(
        t,
        "scope.signatures.has",
        scope.signatures.has.len(),
        want_scope.signatures.has.len(),
    );
    let fields: [(&'static str, bool); 28] = [
        (
            "sessionSymbols",
            api_stability_values_equal(
                t,
                "sessionSymbols",
                session_symbols.len(),
                want_session_symbols.len(),
            ),
        ),
        (
            "sessionTypes",
            api_stability_values_equal(
                t,
                "sessionTypes",
                session_types.len(),
                want_session_types.len(),
            ),
        ),
        (
            "sessionComponents",
            api_stability_values_equal(
                t,
                "sessionComponents",
                session_components.len(),
                want_session_components.len(),
            ),
        ),
        (
            "sessionSignatures",
            api_stability_values_equal(
                t,
                "sessionSignatures",
                session_signatures.len(),
                want_session_signatures.len(),
            ),
        ),
        (
            "typeFrames",
            api_stability_values_equal(t, "typeFrames", type_frames.len(), want_type_frames.len()),
        ),
        (
            "signatureFrames",
            api_stability_values_equal(
                t,
                "signatureFrames",
                signature_frames.len(),
                want_signature_frames.len(),
            ),
        ),
        (
            "carriers",
            api_stability_values_equal(t, "carriers", carriers.len(), want_carriers.len()),
        ),
        (
            "settleSymbols",
            api_stability_values_equal(
                t,
                "settleSymbols",
                settle_symbols.len(),
                want_settle_symbols.len(),
            ),
        ),
        (
            "settleTypes",
            api_stability_values_equal(
                t,
                "settleTypes",
                settle_types.len(),
                want_settle_types.len(),
            ),
        ),
        (
            "settleComponents",
            api_stability_values_equal(
                t,
                "settleComponents",
                settle_components.len(),
                want_settle_components.len(),
            ),
        ),
        (
            "settleSignatures",
            api_stability_values_equal(
                t,
                "settleSignatures",
                settle_signatures.len(),
                want_settle_signatures.len(),
            ),
        ),
        ("scope", scope_equal),
        (
            "publishTypes",
            api_stability_values_equal(
                t,
                "publishTypes",
                publish_types.len(),
                want_publish_types.len(),
            ),
        ),
        (
            "publishSignatures",
            api_stability_values_equal(
                t,
                "publishSignatures",
                publish_signatures.len(),
                want_publish_signatures.len(),
            ),
        ),
        (
            "activeSymbols",
            api_stability_values_equal(
                t,
                "activeSymbols",
                active_symbols.len(),
                want_active_symbols.len(),
            ),
        ),
        (
            "activeTypes",
            api_stability_values_equal(
                t,
                "activeTypes",
                active_types.len(),
                want_active_types.len(),
            ),
        ),
        (
            "activeComponents",
            api_stability_values_equal(
                t,
                "activeComponents",
                active_components.len(),
                want_active_components.len(),
            ),
        ),
        (
            "activeSignatures",
            api_stability_values_equal(
                t,
                "activeSignatures",
                active_signatures.len(),
                want_active_signatures.len(),
            ),
        ),
        (
            "activeAliases",
            api_stability_values_equal(
                t,
                "activeAliases",
                active_aliases.len(),
                want_active_aliases.len(),
            ),
        ),
        (
            "activeConditionals",
            api_stability_values_equal(
                t,
                "activeConditionals",
                active_conditionals.len(),
                want_active_conditionals.len(),
            ),
        ),
        (
            "signatureArgs",
            api_stability_values_equal(
                t,
                "signatureArgs",
                signature_args.len(),
                want_signature_args.len(),
            ),
        ),
        (
            "signatureParameterCache",
            api_stability_values_equal(
                t,
                "signatureParameterCache",
                signature_parameter_cache.len(),
                want_signature_parameter_cache.len(),
            ),
        ),
        (
            "safetySafeMemo",
            api_stability_values_equal(
                t,
                "safetySafeMemo",
                safety_safe_memo.len(),
                want_safety_safe_memo.len(),
            ),
        ),
        (
            "typeNameSymbols",
            api_stability_values_equal(
                t,
                "typeNameSymbols",
                type_name_symbols.len(),
                want_type_name_symbols.len(),
            ),
        ),
        (
            "optionalTaggedSymbols",
            api_stability_values_equal(
                t,
                "optionalTaggedSymbols",
                optional_tagged_symbols.len(),
                want_optional_tagged_symbols.len(),
            ),
        ),
        ("work", work == want_work),
        ("safetyWork", safety_work == want_safety_work),
        (
            "materializationReads",
            api_stability_values_equal(
                t,
                "materializationReads",
                materialization_reads.len(),
                want_materialization_reads.len(),
            ),
        ),
    ];
    let mut diff: Vec<&'static str> =
        fields.iter().filter(|(_, equal)| !equal).map(|(name, _)| *name).collect();
    if materialization_epoch != want_materialization_epoch {
        diff.push("materializationEpoch");
    }
    diff
}

// go test: internal/typeparser/api_stability_reset_test.go:29 apiStabilityValuesEqual 176c1eb48c4a9148
// Rust: Go's reflective comparison of one collection field (map or slice): only one side may be non-empty, and the
// field is equal when both are empty. Integer fields are compared directly in `api_stability_analysis_diff`.
fn api_stability_values_equal(t: &T, path: &str, got: usize, want: usize) -> bool {
    t.helper();
    if got != 0 && want != 0 {
        t.fatal(ets_go::sprintf!("%s: both sides are non-empty", path));
    }
    got == 0 && want == 0
}

// go test: internal/typeparser/api_stability_reset_test.go:54 apiStabilityResetSource 3bc65979fd0024c9
const API_STABILITY_RESET_SOURCE: &str = "/** @stability experimental */
interface E { value: string }
/** @stability unstable */
interface U { value: string }
export interface Box<A> { value: A; map<B>(f: (a: A) => B): Box<B> }
export interface Node { next: Node; payload: E }
export type Pick2<A, K extends keyof A> = { [P in K]: A[P] }
export type IfE<A> = A extends E ? E : U
export declare const make: <A>(a: A) => Box<A>
export declare const boxed: Box<E>
export declare class Service extends Box<U> { run(): IfE<string> }
export interface Leaky { e: E }
export interface Other { u: U }
/** @stability unstable */
interface Reportable {
/** @stability experimental */ test?: string }
declare global { interface Error extends Reportable {} }
export function augmented(e: Error) {}
";

// go test: internal/typeparser/api_stability_reset_test.go:76 TestApiStabilityResetMatchesFreshAnalysis 91a9bd4d0276c5b5
// TestApiStabilityResetMatchesFreshAnalysis proves a reset analysis is
// indistinguishable from a new one after a query that fills its tables.
#[test]
fn test_api_stability_reset_matches_fresh_analysis() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_internal(t, API_STABILITY_RESET_SOURCE);
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let mut session = tp.new_api_stability_session();
        for name in ["Box", "Node", "Pick2", "IfE", "make", "boxed", "Service", "augmented"] {
            let symbol = api_stability_test_export(t, tp.checker, sf, name);
            session.used_by_symbol(&mut tp, symbol);
        }

        let dirty =
            api_stability_analysis_diff(t, &session.analysis, &new_api_stability_analysis());
        for field in [
            "sessionSymbols",
            "sessionTypes",
            "sessionComponents",
            "sessionSignatures",
            "typeFrames",
            "signatureFrames",
            "carriers",
            "settleSymbols",
            "settleTypes",
            "settleComponents",
            "settleSignatures",
            "signatureArgs",
            "signatureParameterCache",
            "safetySafeMemo",
            "typeNameSymbols",
            "optionalTaggedSymbols",
            "work",
            "safetyWork",
            "materializationReads",
            "materializationEpoch",
        ] {
            if !dirty.contains(&field) {
                t.error(ets_go::sprintf!(
                    "the queries left %s untouched, so the reset of it is not exercised (dirty: %v)",
                    field,
                    dirty
                ));
            }
        }

        session.reset();
        let diff = api_stability_analysis_diff(t, &session.analysis, &new_api_stability_analysis());
        if !diff.is_empty() {
            t.fatal(ets_go::sprintf!("reset analysis differs from a fresh one in %v", diff));
        }
    });
}

// go test: internal/typeparser/api_stability_reset_test.go:110 TestApiStabilityResetHidesPreviousExport 71102a6a6f42e946
// TestApiStabilityResetHidesPreviousExport proves no state of one export
// reaches the next one: an exhausted budget and a stale result stored for the
// next root are both discarded by Reset.
#[test]
fn test_api_stability_reset_hides_previous_export() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_internal(t, API_STABILITY_RESET_SOURCE);
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let leaky = api_stability_test_export(t, tp.checker, sf, "Leaky");
        let other = api_stability_test_export(t, tp.checker, sf, "Other");
        let mut session = tp.new_api_stability_session();
        let got = api_stability_dependency_names(&session.used_by_symbol(&mut tp, leaky));
        if got != ["E"] {
            t.fatal(ets_go::sprintf!("Leaky = %v, want [E]", got));
        }

        session.analysis.work = API_STABILITY_MAX_WORK;
        session.analysis.safety_work = API_STABILITY_SAFETY_MAX_WORK + 1;
        session.analysis.materialization_epoch = 99;
        session.analysis.session_symbols.insert(other, new_api_stability_surface());

        session.reset();
        let used = session.used_by_symbol(&mut tp, other);
        let got = api_stability_dependency_names(&used);
        if used.incomplete || got != ["U"] {
            t.fatal(ets_go::sprintf!(
                "Other after reset = %v (incomplete %v), want complete [U]",
                got,
                used.incomplete
            ));
        }
        if session.analysis.session_symbols.contains_key(&leaky) {
            t.fatal("the previous export's symbol result survived the reset");
        }
    });
}

// go test: internal/typeparser/api_stability_reset_test.go:143 TestApiStabilityResetSharedHitIsNotRepublished 9c465c47bee417c2
// TestApiStabilityResetSharedHitIsNotRepublished proves a later export reuses
// a published surface after a reset: the composed result is the same, the hit
// is recorded for settlement, it is not queued for publication again, and the
// snapshot keeps the findings it was published with.
#[test]
fn test_api_stability_reset_shared_hit_is_not_republished() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Tagged { value: string }
export interface Box<A> { value: A }
export interface First { box: Box<E>; tagged: Tagged }
export interface Second { box: Box<E>; tagged: Tagged }
",
        );
        prime_api_stability_checker(t, &mut compiled);
        let mut tp = compiled.type_parser();

        let first = api_stability_test_export(t, tp.checker, sf, "First");
        let box_member = first
            .members()
            .and_then(|members| SymbolTable::get(&members, "box"))
            .expect("First.box");
        let box_type = tp.checker.get_type_of_symbol_exported(box_member);
        let tagged_symbol = api_stability_test_export(t, tp.checker, sf, "Tagged");
        let tagged = tp.checker.get_declared_type_of_symbol_exported(tagged_symbol);
        let mut session = tp.new_api_stability_session();
        let got = api_stability_dependency_names(&session.used_by_symbol(&mut tp, first));
        if got != ["E", "Tagged"] {
            t.fatal(ets_go::sprintf!("First = %v, want [E Tagged]", got));
        }
        let box_entry =
            api_stability_shared_type_entry(&tp, Some(box_type), ApiStabilityInspection::Shallow);
        let Some(box_entry) = box_entry.filter(|entry| entry.findings.len() == 1) else {
            t.fatal("Box<E> snapshot missing, want one retained finding");
        };
        let tagged_entry =
            api_stability_shared_type_entry(&tp, Some(tagged), ApiStabilityInspection::Shallow);
        if !tagged_entry
            .as_ref()
            .is_some_and(|entry| entry.findings.is_empty() && entry.removed.len() == 1)
        {
            t.fatal("Tagged snapshot does not hold only its own tag, removed");
        }

        session.reset();
        let box_key = ApiStabilityTypeKey {
            t: box_type,
            inspect: ApiStabilityInspection::Shallow,
            carrier: None,
        };
        let surface = session.analysis.type_surface(
            &mut tp,
            Some(box_type),
            ApiStabilityInspection::Shallow,
            &ApiStabilitySubstitution::default(),
        );
        if surface.findings.len() != 1 || session.analysis.work != 0 {
            t.fatal(ets_go::sprintf!(
                "Box<E> hit = %d findings after %d work, want 1 finding and no work",
                surface.findings.len() as i64,
                session.analysis.work
            ));
        }
        if session.analysis.settle_types != [box_key] {
            t.fatal(ets_go::sprintf!(
                "settleTypes has %d entries, want the Box<E> hit",
                session.analysis.settle_types.len() as i64
            ));
        }
        if !session.analysis.publish_types.is_empty() {
            t.fatal(ets_go::sprintf!(
                "the hit was queued for publication: %d entries",
                session.analysis.publish_types.len() as i64
            ));
        }

        session.reset();
        let second = api_stability_test_export(t, tp.checker, sf, "Second");
        let got = api_stability_dependency_names(&session.used_by_symbol(&mut tp, second));
        if got != ["E", "Tagged"] {
            t.fatal(ets_go::sprintf!("Second after reset = %v, want [E Tagged]", got));
        }
        // Rust: Go compares the snapshot pointer; the Rust entry is read by value, so an unchanged snapshot is
        // the same contents (the first entry for a key is never replaced).
        let after =
            api_stability_shared_type_entry(&tp, Some(box_type), ApiStabilityInspection::Shallow);
        if after.as_ref() != Some(&box_entry) || box_entry.findings.len() != 1 {
            t.fatal("reuse replaced or mutated the Box<E> snapshot");
        }
    });
}
