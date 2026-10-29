// Ported from effect-tsgo's internal/typeparser/data_first_signature_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::{Signature, SignatureKind, TypeFormatFlags};
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/data_first_signature_test.go:14 TestDerivedPipeableSignature_Provide 8c77a5491b25f836
#[test]
fn test_derived_pipeable_signature_provide() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect, Layer, ServiceMap } from "effect"

class MyService extends ServiceMap.Service<MyService>()("MyService", {
  make: Effect.succeed({ value: 1 })
}) {
  static Default = Layer.effect(this, this.make)
}

declare const program: Effect.Effect<number, never, "ProgramEnv">

const provided = Effect.provide(program, MyService.Default, { local: true })
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "provided");
        log_derived_pipeable_signature_comparison(t, &mut tp, Some(call));
        let Some(result) = tp.data_first_or_last_call(call) else {
            t.fatal("expected provide call to normalize via derived signature comparison");
        };
        if result.subject_index != 0 {
            t.fatal(sprintf!(
                "expected provide subject index 0, got %d",
                result.subject_index as i64
            ));
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:46 TestDerivedPipeableSignature_LayerSucceed 282be845690aba7c
#[test]
fn test_derived_pipeable_signature_layer_succeed() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Layer, ServiceMap } from "effect"

class Service extends ServiceMap.Service<Service, {
  readonly value: 1
}>()("Service") {}

declare const make: { readonly value: 1 }

const live = Layer.succeed(Service, make)
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "live");
        log_derived_pipeable_signature_comparison(t, &mut tp, Some(call));
        let Some(result) = tp.data_first_or_last_call(call) else {
            t.fatal("expected Layer.succeed call to normalize via derived signature comparison");
        };
        if result.subject_index != 1 {
            t.fatal(sprintf!(
                "expected Layer.succeed subject index 1, got %d",
                result.subject_index as i64
            ));
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:76 TestPipingFlows_DataFirstCalls 2ff4e4b942ece7f1
#[test]
fn test_piping_flows_data_first_calls() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect, Layer, ServiceMap } from "effect"

class MyService extends ServiceMap.Service<MyService>()("MyService", {
  make: Effect.succeed({ value: 1 as const })
}) {
  static Default = Layer.effect(this, this.make)
}

declare const program: Effect.Effect<number, never, "ProgramEnv">
declare const make: { readonly value: 1 }

export const provided = Effect.provide(program, MyService.Default, { local: true })
export const live = Layer.succeed(MyService, make)
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let flows = tp.piping_flows(sf, false);
        let provided_call = find_variable_initializer_call_by_name(t, sf, "provided");
        let live_call = find_variable_initializer_call_by_name(t, sf, "live");

        let provided_flow = find_flow_by_node(t, sf, flows.as_deref(), provided_call);
        if node_text(Some(sf), provided_flow.partial.subject.node).trim() != "program" {
            t.fatal(sprintf!(
                "provided subject = %q, want %q",
                node_text(Some(sf), provided_flow.partial.subject.node).trim(),
                "program"
            ));
        }
        assert_single_transformation(
            t,
            sf,
            Some(provided_flow),
            TransformationKind::DataFirst,
            "Effect.provide",
            &["MyService.Default", "{ local: true }"],
        );
        let got = strip_whitespace(&reconstruct_piping_flow(
            sf,
            Some(&provided_flow.partial.subject),
            &ptr_transformations(&provided_flow.partial.transformations),
        ));
        if got != strip_whitespace("Effect.provide(MyService.Default, { local: true })(program)") {
            t.fatal(sprintf!("provided reconstructed flow = %q", got));
        }

        let live_flow = find_flow_by_node(t, sf, flows.as_deref(), live_call);
        if node_text(Some(sf), live_flow.partial.subject.node).trim() != "make" {
            t.fatal(sprintf!(
                "live subject = %q, want %q",
                node_text(Some(sf), live_flow.partial.subject.node).trim(),
                "make"
            ));
        }
        assert_single_transformation(
            t,
            sf,
            Some(live_flow),
            TransformationKind::DataLast,
            "Layer.succeed",
            &["MyService"],
        );
        let got = strip_whitespace(&reconstruct_piping_flow(
            sf,
            Some(&live_flow.partial.subject),
            &ptr_transformations(&live_flow.partial.transformations),
        ));
        if got != strip_whitespace("Layer.succeed(MyService)(make)") {
            t.fatal(sprintf!("live reconstructed flow = %q", got));
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:121 TestPipingFlows_CallWithOptionals ad6516d05773e99e
#[test]
fn test_piping_flows_call_with_optionals() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

declare const program: Effect.Effect<number>
declare const signal: AbortSignal

export const run = Effect.runPromise(program, { signal })
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let flows = tp.piping_flows(sf, false);
        let run_flow = find_flow_by_node(
            t,
            sf,
            flows.as_deref(),
            find_variable_initializer_call_by_name(t, sf, "run"),
        );
        assert_single_transformation(
            t,
            sf,
            Some(run_flow),
            TransformationKind::CallWithOptionals,
            "Effect.runPromise",
            &["{ signal }"],
        );
        let got = strip_whitespace(&reconstruct_piping_flow(
            sf,
            Some(&run_flow.partial.subject),
            &ptr_transformations(&run_flow.partial.transformations),
        ));
        if got != strip_whitespace("Effect.runPromise(program, { signal })") {
            t.fatal(sprintf!("run reconstructed flow = %q", got));
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:144 TestPipingFlows_TypeArguments 61662b53d3dd6787
#[test]
fn test_piping_flows_type_arguments() {
    run(|t| {
        t.parallel();

        let source = r#"
import { pipe } from "effect"

declare function transform<A, B>(self: A, f: (value: A) => B): B
declare function transform<A, B>(f: (value: A) => B): (self: A) => B
declare function identity<A>(value: A): A

export const piped = pipe("value", transform<string, number>((value) => value.length))
export const dataFirst = transform<string, number>("value", (value) => value.length)
export const curried = transform<string, number>((value) => value.length)("value")
export const called = identity<string>("value")
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        struct Test {
            name: &'static str,
            want: &'static [&'static str],
        }
        for test in [
            Test { name: "piped", want: &["string", "number"] },
            Test { name: "dataFirst", want: &["string", "number"] },
            Test { name: "curried", want: &["string", "number"] },
            Test { name: "called", want: &["string"] },
        ] {
            let call = find_variable_initializer_call_by_name(t, sf, test.name);
            let flows = tp.piping_flows(sf, false);
            let flow = find_flow_by_node(t, sf, flows.as_deref(), call);
            if flow.partial.transformations.is_empty() {
                t.fatal(sprintf!("%s transformation count = 0, want at least 1", test.name));
            }
            let transformation =
                &flow.partial.transformations[flow.partial.transformations.len() - 1];
            let Some(type_arguments) = transformation.type_arguments else {
                t.fatal(sprintf!("%s type arguments are nil", test.name));
            };
            let mut got: Vec<String> = Vec::with_capacity(type_arguments.nodes().len());
            for &type_argument in type_arguments.nodes() {
                got.push(node_text(Some(sf), Some(type_argument)).trim().to_string());
            }
            if got != test.want {
                t.fatal(sprintf!(
                    "%s type arguments = %v, want %v",
                    test.name,
                    got,
                    test.want.to_vec()
                ));
            }
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:191 TestPipingFlows_DecomposesOnlyCurriedPipeableOverloads 5c5976e1c9f2ddd2
#[test]
fn test_piping_flows_decomposes_only_curried_pipeable_overloads() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

declare const program: Effect.Effect<number, string>

export const caught = Effect.catch(() => Effect.succeed(0))(program)
export const named = Effect.fn("named")(function*() {
  return 1
})
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let caught_call = find_variable_initializer_call_by_name(t, sf, "caught");
        let Some(caught_flow) = tp.longest_piping_flow_at(caught_call, false) else {
            t.fatal("expected curried catch flow");
        };
        let got = node_text(Some(sf), caught_flow.partial.subject.node).trim().to_string();
        if got != "program" {
            t.fatal(sprintf!("caught subject = %q, want program", got));
        }
        assert_single_transformation(
            t,
            sf,
            Some(&caught_flow),
            TransformationKind::Call,
            "Effect.catch",
            &["() => Effect.succeed(0)"],
        );

        let named_call = find_variable_initializer_call_by_name(t, sf, "named");
        let Some(named_flow) = tp.longest_piping_flow_at(named_call, false) else {
            t.fatal("expected named Effect.fn call flow");
        };
        assert_single_transformation(
            t,
            sf,
            Some(&named_flow),
            TransformationKind::Call,
            r#"Effect.fn("named")"#,
            &[],
        );
    });
}

// go test: internal/typeparser/data_first_signature_test.go:226 TestPipingFlows_TransformationOccurrencesAreUnique 8f7bcdcc6824aaf2
#[test]
fn test_piping_flows_transformation_occurrences_are_unique() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect, pipe } from "effect"

declare const base: Effect.Effect<number, string>

export const piped = pipe(
  base,
  Effect.map((value) => value + 1),
  Effect.flatMap((value) => Effect.succeed(value * 2))
)

export const nested = Effect.map(Effect.succeed(1), (value) => value + 1)

export const effectFn = Effect.fn(
  function*() {
    return 1
  },
  Effect.map((value) => value + 1),
  Effect.catch(() => Effect.succeed(0))
)
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let mut seen: FxHashMap<P<Node>, usize> = FxHashMap::default();
        let flows = tp.piping_flows(sf, true);
        for (flow_index, flow) in flows.as_deref().unwrap_or_default().iter().enumerate() {
            for transformation in &flow.partial.transformations {
                if let Some(&previous_flow_index) = seen.get(&transformation.callee) {
                    t.fatal(sprintf!(
                        "transformation %q occurs in flows %d and %d",
                        node_text(Some(sf), Some(transformation.callee)),
                        previous_flow_index as i64,
                        flow_index as i64
                    ));
                }
                seen.insert(transformation.callee, flow_index);
            }
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:270 TestPipingFlows_SkipsParentheses 298eb93c7c829715
#[test]
fn test_piping_flows_skips_parentheses() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect, Option } from "effect"

export const some = Effect.succeed(((Option.some(1))))
export const none = Effect.succeed(((Option.none())))
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let some_call = find_variable_initializer_call_by_name(t, sf, "some");
        let some_flows = tp.piping_flows(sf, false);
        let some_flow = find_flow_by_node(t, sf, some_flows.as_deref(), some_call);
        let got = node_text(Some(sf), some_flow.partial.subject.node).trim().to_string();
        if got != "1" {
            t.fatal(sprintf!("some subject = %q, want %q", got, "1"));
        }
        if some_flow.partial.transformations.len() != 2 {
            t.fatal(sprintf!(
                "some transformation count = %d, want 2",
                some_flow.partial.transformations.len() as i64
            ));
        }
        let got = node_text(Some(sf), Some(some_flow.partial.transformations[0].callee))
            .trim()
            .to_string();
        if got != "Option.some" {
            t.fatal(sprintf!("first some transformation = %q, want %q", got, "Option.some"));
        }
        let got = node_text(Some(sf), Some(some_flow.partial.transformations[1].callee))
            .trim()
            .to_string();
        if got != "Effect.succeed" {
            t.fatal(sprintf!("second some transformation = %q, want %q", got, "Effect.succeed"));
        }

        let none_call = find_variable_initializer_call_by_name(t, sf, "none");
        let none_flows = tp.piping_flows(sf, false);
        let none_flow = find_flow_by_node(t, sf, none_flows.as_deref(), none_call);
        let got = node_text(Some(sf), none_flow.partial.subject.node).trim().to_string();
        if got != "Option.none()" {
            t.fatal(sprintf!("none subject = %q, want %q", got, "Option.none()"));
        }
        assert_single_transformation(
            t,
            sf,
            Some(none_flow),
            TransformationKind::Call,
            "Effect.succeed",
            &[],
        );
    });
}

// go test: internal/typeparser/data_first_signature_test.go:306 TestLongestPipingFlowAt_ReturnsLongestNestedFlow e9363406bdee1ac2
#[test]
fn test_longest_piping_flow_at_returns_longest_nested_flow() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect } from "effect"

declare const work: Effect.Effect<string>

export const dataFirst = Effect.raceFirst(
  Effect.flatMap(Effect.sleep("1 second"), () => Effect.fail("timeout")),
  work
)

export const pipeable = Effect.raceFirst(
  work,
  ((Effect.sleep("2 seconds").pipe(
    Effect.andThen(Effect.succeed("fallback"))
  )))
)
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let data_first_race = find_variable_initializer_call_by_name(t, sf, "dataFirst");
        if tp
            .data_first_or_last_call(data_first_race)
            .is_none_or(|parsed| parsed.subject_index != 0)
        {
            t.fatal(
                "expected data-first Effect.raceFirst with omitted options to normalize around self",
            );
        }
        let data_first_timer = data_first_race.as_call_expression().arguments().nodes()[0];
        let Some(data_first_flow) = tp.longest_piping_flow_at(data_first_timer, false) else {
            t.fatal("expected data-first timer sub-flow");
        };
        if data_first_flow.node != data_first_timer {
            t.fatal("data-first sub-flow should be rooted at the requested node");
        }
        let got = transformation_callees(sf, &data_first_flow);
        if got != ["Effect.sleep", "Effect.flatMap"] {
            t.fatal(sprintf!("data-first transformations = %v", got));
        }

        let pipeable_race = find_variable_initializer_call_by_name(t, sf, "pipeable");
        let pipeable_timer = pipeable_race.as_call_expression().arguments().nodes()[1];
        let Some(pipeable_flow) = tp.longest_piping_flow_at(pipeable_timer, false) else {
            t.fatal("expected parenthesized pipeable timer sub-flow");
        };
        let got = transformation_callees(sf, &pipeable_flow);
        if got != ["Effect.sleep", "Effect.andThen"] {
            t.fatal(sprintf!("pipeable transformations = %v", got));
        }

        let Some(outer_flow) = tp.longest_piping_flow_at(data_first_race, false) else {
            t.fatal("expected complete outer flow");
        };
        let got = transformation_callees(sf, &outer_flow);
        if got != ["Effect.sleep", "Effect.flatMap", "Effect.raceFirst"] {
            t.fatal(sprintf!("outer transformations = %v", got));
        }

        let Some(mut prefix) = PipingFlow::copy_prefix(Some(&outer_flow), 2) else {
            t.fatal("expected a two-transformation prefix");
        };
        // Rust: Go compares the subject structs (the node and the lazy type's pointer); the lazy handle exposes
        // no identity outside its module, so the node and the presence of the lazy type are compared.
        if prefix.subject.node != outer_flow.partial.subject.node
            || prefix.subject.out_type.is_some() != outer_flow.partial.subject.out_type.is_some()
        {
            t.fatal("prefix should preserve the flow subject");
        }
        if prefix.transformations.len() != 2 {
            t.fatal(sprintf!(
                "prefix transformation count = %d, want 2",
                prefix.transformations.len() as i64
            ));
        }
        let got = node_text(Some(sf), Some(prefix.transformations[1].callee)).trim().to_string();
        if got != "Effect.flatMap" {
            t.fatal(sprintf!("last prefix transformation = %q, want Effect.flatMap", got));
        }
        let original_kind = outer_flow.partial.transformations[0].kind;
        // Rust: TransformationKind is a closed enum, so a kind the first step of this flow never has stands in for
        // Go's `TransformationKind("test")`.
        prefix.transformations[0].kind = TransformationKind::EffectFnUntraced;
        if outer_flow.partial.transformations[0].kind != original_kind {
            t.fatal("prefix transformations should have an independent backing slice");
        }
        if PipingFlow::copy_prefix(Some(&outer_flow), 0)
            .is_none_or(|empty| !empty.transformations.is_empty())
        {
            t.fatal("expected an empty prefix rooted at the flow subject");
        }
        if PipingFlow::copy_prefix(Some(&outer_flow), -1).is_some()
            || PipingFlow::copy_prefix(
                Some(&outer_flow),
                outer_flow.partial.transformations.len() as i64 + 1,
            )
            .is_some()
        {
            t.fatal("out-of-bounds prefixes should return nil");
        }
        let nil_flow: Option<&PipingFlow> = None;
        if PipingFlow::copy_prefix(nil_flow, 0).is_some() {
            t.fatal("a nil complete flow should return a nil prefix");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:394 transformationCallees 67a545e417da150a
fn transformation_callees(sf: P<SourceFile>, flow: &PipingFlow) -> Vec<String> {
    let mut result = Vec::with_capacity(flow.partial.transformations.len());
    for transformation in &flow.partial.transformations {
        result.push(node_text(Some(sf), Some(transformation.callee)).trim().to_string());
    }
    result
}

// go test: internal/typeparser/data_first_signature_test.go:402 TestDataFirstOrLastCall_OptionMatchV4 0a5fe90f14bd591e
#[test]
fn test_data_first_or_last_call_option_match_v4() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Effect, Option } from "effect"

declare const value: Option.Option<number>

export const result = Option.match(value, {
  onNone: () => Effect.fail("missing"),
  onSome: Effect.succeed
})
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "result");
        let Some(result) = tp.data_first_or_last_call(call) else {
            t.fatal("expected Option.match data-first call to normalize");
        };
        if result.subject_index != 0 {
            t.fatal(sprintf!("subject index = %d, want 0", result.subject_index as i64));
        }
        let got = node_text(Some(sf), Some(result.subject)).trim().to_string();
        if got != "value" {
            t.fatal(sprintf!("subject = %q, want %q", got, "value"));
        }
        let got = node_text(Some(sf), Some(result.callee)).trim().to_string();
        if got != "Option.match" {
            t.fatal(sprintf!("callee = %q, want %q", got, "Option.match"));
        }
        if result.args.len() != 1
            || !node_text(Some(sf), Some(result.args[0])).trim().starts_with('{')
        {
            t.fatal("expected handlers object as sole transformation argument");
        }

        let flows = tp.piping_flows(sf, false);
        let flow = find_flow_by_node(t, sf, flows.as_deref(), call);
        let got = node_text(Some(sf), flow.partial.subject.node).trim().to_string();
        if got != "value" {
            t.fatal(sprintf!("flow subject = %q, want %q", got, "value"));
        }
        assert_single_transformation(
            t,
            sf,
            Some(flow),
            TransformationKind::DataFirst,
            "Option.match",
            &[r#"{
  onNone: () => Effect.fail("missing"),
  onSome: Effect.succeed
}"#],
        );
    });
}

// go test: internal/typeparser/data_first_signature_test.go:447 TestDataFirstOrLastCall_AlphaEquivalentOverloads 7cbab7d4c8b289e9
#[test]
fn test_data_first_or_last_call_alpha_equivalent_overloads() {
    run(|t| {
        t.parallel();

        let source = "
interface Box<A> { readonly value: A }
interface Pair<A, B> { readonly left: A; readonly right: B }

declare function reordered<A, B, C = B>(self: Box<A>, options: {
  readonly onEmpty: () => B
  readonly onValue: (value: A) => C
}): B | C
declare function reordered<B, A, C = B>(options: {
  readonly onValue: (value: A) => C
  readonly onEmpty: () => B
}): (self: Box<A>) => C | B

declare function moved<A>(self: Box<A>, index: number): A
declare function moved(index: number): <Value>(self: Box<Value>) => Value

declare function mapped<A extends object, N extends string, B>(self: A, name: N, value: B): {
  [K in N | keyof A]: K extends keyof A ? A[K] : B
}
declare function mapped<N extends string, A extends object, B>(name: N, value: B): (self: A) => {
  [K in N | keyof A]: K extends keyof A ? A[K] : B
}

declare function distinctRoles<A, B>(self: Box<A>, f: (value: A) => B): Pair<A, B>
declare function distinctRoles<A, B>(f: (value: B) => A): (self: Box<A>) => Pair<A, B>

declare function differentBinders<T>(self: Box<T>, f: (value: T) => T): T | T
declare function differentBinders<U, V>(f: (value: U) => V): (self: Box<U>) => U | V

namespace Left { export interface Same<A> { readonly value: A } }
namespace Right { export interface Same<A> { readonly value: A } }
declare function differentSymbols<A>(self: Left.Same<A>, size: number): A
declare function differentSymbols<A>(size: number): (self: Right.Same<A>) => A

declare function differentDefault<A, B = A>(self: Box<A>, f: (value: A) => B): B
declare function differentDefault<A, B = string>(f: (value: A) => B): (self: Box<A>) => B

declare function differentReadonly<A, B>(self: Box<A>, options: { readonly f: (value: A) => B }): B
declare function differentReadonly<A, B>(options: { f: (value: A) => B }): (self: Box<A>) => B

declare function optionalSubject<A>(self: Box<A>, size: number): A
declare function optionalSubject(size: number): <A>(self?: Box<A>) => A

declare const box: Box<number>
declare const left: Left.Same<number>
declare const object: { readonly a: number }

export const reorderedResult = reordered(box, {
  onEmpty: () => \"empty\",
  onValue: (value) => value
})
export const movedResult = moved(box, 0)
export const mappedResult = mapped(object, \"b\", true)
export const distinctRolesResult = distinctRoles(box, String)
export const differentBindersResult = differentBinders(box, (value) => value)
export const differentSymbolsResult = differentSymbols(left, 1)
export const differentDefaultResult = differentDefault(box, String)
export const differentReadonlyResult = differentReadonly(box, { f: String })
export const optionalSubjectResult = optionalSubject(box, 1)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        for name in ["reorderedResult", "movedResult", "mappedResult"] {
            let call = find_variable_initializer_call_by_name(t, sf, name);
            let Some(result) = tp.data_first_or_last_call(call) else {
                t.error(ets_go::sprintf!("expected %s to normalize", name));
                continue;
            };
            if result.subject_index != 0 {
                t.error(ets_go::sprintf!(
                    "%s subject index = %d, want 0",
                    name,
                    result.subject_index as i64
                ));
            }
        }

        for name in [
            "distinctRolesResult",
            "differentBindersResult",
            "differentSymbolsResult",
            "differentDefaultResult",
            "differentReadonlyResult",
            "optionalSubjectResult",
        ] {
            let call = find_variable_initializer_call_by_name(t, sf, name);
            if tp.data_first_or_last_call(call).is_some() {
                t.error(ets_go::sprintf!("expected %s not to normalize", name));
            }
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:541 TestDataFirstOrLastCall_CallableAlias 822c0cf656633663
#[test]
fn test_data_first_or_last_call_callable_alias() {
    run(|t| {
        t.parallel();

        let source = "
type Pipeable = (self: string) => number

declare function operation(self: string, size: number): number
declare function operation(size: number): Pipeable

export const result = operation(\"value\", 1)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "result");
        let Some(result) = tp.data_first_or_last_call(call) else {
            t.fatal("expected a pipeable overload returned through a callable alias to normalize");
        };
        if result.subject_index != 0 {
            t.fatal(ets_go::sprintf!("subject index = %d, want 0", result.subject_index as i64));
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:566 TestDataFirstOrLastCall_InferredParameterTypes 9067a1ab8a7b5fdc
#[test]
fn test_data_first_or_last_call_inferred_parameter_types() {
    run(|t| {
        t.parallel();

        let source = "
declare function operation(self: string, size): number
declare function operation(size): (self: string) => number

export const result = operation(\"value\", 1)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "result");
        if tp.data_first_or_last_call(call).is_none_or(|result| result.subject_index != 0) {
            t.fatal("expected overloads with inferred any parameter types to normalize");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:585 TestPipeableSignatureShapeWithoutExplicitDeclarations 845260f6589de5ca
#[test]
fn test_pipeable_signature_shape_without_explicit_declarations() {
    run(|t| {
        t.parallel();

        let source = r#"
declare function operation(self: string, size: number): number
declare function operation(size: number): (self: string) => number

export const result = operation("value", 1)
"#;

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();
        let call = find_variable_initializer_call_by_name(t, sf, "result");
        let resolved_signature = tp.checker.get_resolved_signature_exported(call);
        let resolved = raw_signature(Some(resolved_signature));
        let callee_type = tp.get_type_at_location(call.as_call_expression().expression());
        let candidates = callee_type.map_or(&[][..], |callee_type| {
            tp.checker.get_signatures_of_type_exported(callee_type, SignatureKind::Call)
        });
        let Some(resolved) = resolved.filter(|_| candidates.len() == 2) else {
            t.fatal("expected both overload signatures");
        };
        let mut pipeable = raw_signature(Some(candidates[0])).unwrap();
        if pipeable.parameters().len() != 1 {
            pipeable = raw_signature(Some(candidates[1])).unwrap();
        }

        let resolved_return = tp.checker.get_return_type_of_signature_exported(resolved);
        let synthetic_data_first = tp.checker.new_call_signature(
            resolved.type_parameters(),
            resolved.this_parameter(),
            resolved.parameters(),
            resolved_return,
        );
        let pipeable_return = tp.checker.get_return_type_of_signature_exported(pipeable);
        let synthetic_pipeable = tp.checker.new_call_signature(
            pipeable.type_parameters(),
            pipeable.this_parameter(),
            pipeable.parameters(),
            pipeable_return,
        );
        if !synthetic_data_first.declaration().is_some_and(ets_shim::ast::node_is_synthesized)
            || !synthetic_pipeable.declaration().is_some_and(ets_shim::ast::node_is_synthesized)
        {
            t.fatal("expected signatures with no source declaration site");
        }
        if !matches_pipeable_signature(
            &mut tp,
            Some(synthetic_data_first),
            Some(synthetic_pipeable),
            0,
            None,
        ) {
            t.fatal("expected raw signature structures to match without declaration nodes");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:622 TestPipeableSignatureShapeDoesNotInstantiateTypes 9a3bebf0ab064059
#[test]
fn test_pipeable_signature_shape_does_not_instantiate_types() {
    run(|t| {
        t.parallel();

        let source = "
interface Box<A> { readonly value: A }
declare function operation<A, B>(self: Box<A>, f: (value: A) => B): B | A
declare function operation<B, A>(f: (value: A) => B): (self: Box<A>) => A | B
declare const box: Box<number>
export const result = operation(box, String)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();
        let call = find_variable_initializer_call_by_name(t, sf, "result");
        let resolved = tp.checker.get_resolved_signature_exported(call);
        let callee_type = tp.get_type_at_location(call.as_call_expression().expression());
        let candidates = callee_type.map_or(&[][..], |callee_type| {
            tp.checker.get_signatures_of_type_exported(callee_type, SignatureKind::Call)
        });
        let mut pipeable = None;
        for &candidate in candidates {
            let raw = raw_signature(Some(candidate));
            if raw.is_some_and(|raw| raw.parameters().len() == 1) {
                pipeable = Some(candidate);
                break;
            }
        }
        let Some(pipeable) = pipeable else {
            t.fatal("expected data-first and pipeable signatures");
        };
        if !pipeable_signature_shapes_match(&mut tp, Some(resolved), Some(pipeable), 0) {
            t.fatal("expected signatures to be alpha-equivalent");
        }
        if !matches_pipeable_signature(&mut tp, Some(resolved), Some(pipeable), 0, None) {
            t.fatal("expected full matcher to recognize the signatures");
        }
        let before = tp.checker.total_instantiation_count;
        for _ in 0..100 {
            if !compare_pipeable_signature_types(
                &mut tp,
                raw_signature(Some(resolved)),
                raw_signature(Some(pipeable)),
                0,
            ) {
                t.fatal("expected repeated raw type comparison to match");
            }
            if !pipeable_signature_shapes_match(&mut tp, Some(resolved), Some(pipeable), 0) {
                t.fatal("expected cached signature comparison to match");
            }
            if !matches_pipeable_signature(&mut tp, Some(resolved), Some(pipeable), 0, None) {
                t.fatal("expected repeated full signature comparison to match");
            }
        }
        let after = tp.checker.total_instantiation_count;
        if after != before {
            t.fatal(ets_go::sprintf!(
                "type instantiation count changed from %d to %d",
                before as i64,
                after as i64
            ));
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:672 TestPipeableSignatureShapeFailsClosedAtDepthBudget fd013309b1aaec27
#[test]
fn test_pipeable_signature_shape_fails_closed_at_depth_budget() {
    run(|t| {
        t.parallel();

        let mut deep_type = "number".to_string();
        for _ in 0..PIPEABLE_SHAPE_MAX_DEPTH + 2 {
            deep_type = format!("{{ readonly value: {deep_type} }}");
        }
        let source = format!(
            "
declare function operation(self: {deep_type}, size: number): number
declare function operation(size: number): (self: {deep_type}) => number
declare const value: {deep_type}
export const result = operation(value, 1)
"
        );

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, &source);
        let mut tp = compiled.type_parser();
        let call = find_variable_initializer_call_by_name(t, sf, "result");
        if tp.data_first_or_last_call(call).is_some() {
            t.fatal("expected over-budget signature shape to fail closed");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:694 TestDataFirstOrLastCall_RealAlphaEquivalentEffectApis ea3726c385e3ed47
#[test]
fn test_data_first_or_last_call_real_alpha_equivalent_effect_apis() {
    run(|t| {
        t.parallel();

        let source = r#"
import { Exit, Option, Result, UndefinedOr } from "effect"

declare const option: Option.Option<number>
declare const result: Result.Result<number, string>
declare const maybe: number | undefined
declare const exit: Exit.Exit<number, string>

export const optionGetOrElse = Option.getOrElse(option, () => "none")
export const resultMatch = Result.match(result, {
  onFailure: (error) => error.length,
  onSuccess: (value) => value
})
export const undefinedOrMap = UndefinedOr.map(maybe, (value) => String(value))
export const exitMatch = Exit.match(exit, {
  onFailure: (cause) => String(cause),
  onSuccess: (value) => value
})
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        for name in ["optionGetOrElse", "resultMatch", "undefinedOrMap", "exitMatch"] {
            let call = find_variable_initializer_call_by_name(t, sf, name);
            let Some(result) = tp.data_first_or_last_call(call) else {
                t.error(sprintf!("expected %s to normalize", name));
                continue;
            };
            if result.subject_index != 0 {
                t.error(sprintf!(
                    "%s subject index = %d, want 0",
                    name,
                    result.subject_index as i64
                ));
            }
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:733 TestParseDataFirstCallAsPipeable_CatchAllV3 73d496e5f875de72
#[test]
fn test_parse_data_first_call_as_pipeable_catch_all_v3() {
    run(|t| {
        t.parallel();

        let source = r#"
// @effect-v3
import * as Effect from "effect/Effect"

export const shouldReportDataFirst = Effect.catchAll(
  Effect.never,
  () => Effect.log("error")
)
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v3_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "shouldReportDataFirst");
        let Some(result) = tp.data_first_or_last_call(call) else {
            t.fatal("expected data-first catchAll to normalize");
        };
        if node_text(Some(sf), Some(result.subject)).trim() != "Effect.never" {
            t.fatal(sprintf!(
                "subject = %q, want %q",
                node_text(Some(sf), Some(result.subject)).trim(),
                "Effect.never"
            ));
        }
        if node_text(Some(sf), Some(result.callee)).trim() != "Effect.catchAll" {
            t.fatal(sprintf!(
                "callee = %q, want %q",
                node_text(Some(sf), Some(result.callee)).trim(),
                "Effect.catchAll"
            ));
        }
        if result.subject_index != 0 {
            t.fatal(sprintf!("subject index = %d, want 0", result.subject_index as i64));
        }
        if result.args.len() != 1
            || strip_whitespace(&node_text(Some(sf), Some(result.args[0])))
                != strip_whitespace("() => Effect.log(\"error\")")
        {
            t.fatal(sprintf!("args = %q", node_text(Some(sf), Some(result.args[0])).trim()));
        }

        let flows = tp.piping_flows(sf, false);
        let flow = find_flow_by_node(t, sf, flows.as_deref(), call);
        if node_text(Some(sf), flow.partial.subject.node).trim() != "Effect.never" {
            t.fatal(sprintf!(
                "flow subject = %q, want %q",
                node_text(Some(sf), flow.partial.subject.node).trim(),
                "Effect.never"
            ));
        }
        assert_single_transformation(
            t,
            sf,
            Some(flow),
            TransformationKind::DataFirst,
            "Effect.catchAll",
            &["() => Effect.log(\"error\")"],
        );
    });
}

// go test: internal/typeparser/data_first_signature_test.go:776 TestDataFirstOrLastCallRejectsDifferentReturnOrigins 0ef7ff0d8f312b55
#[test]
fn test_data_first_or_last_call_rejects_different_return_origins() {
    run(|t| {
        t.parallel();

        let source = "
type DataFirstResult<T> = { readonly dataFirst: T }
type DataLastResult<T> = { readonly dataLast: T }

declare function operation<T>(self: T, size: number): DataFirstResult<T>
declare function operation(size: number): <T>(self: T) => DataLastResult<T>

const result = operation(\"value\", 2)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "result");
        if tp.data_first_or_last_call(call).is_some() {
            t.fatal("expected overloads with different return origins not to normalize");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:798 TestDataFirstOrLastCallRejectsIncompatibleSubject fdda1f9a370eebba
#[test]
fn test_data_first_or_last_call_rejects_incompatible_subject() {
    run(|t| {
        t.parallel();

        let source = "
type Result = { readonly value: string }

declare function operation(self: string, size: number): Result
declare function operation(size: number): (self: boolean) => Result

const result = operation(\"value\", 2)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "result");
        if tp.data_first_or_last_call(call).is_some() {
            t.fatal("expected overloads with an incompatible subject not to normalize");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:819 TestDataFirstOrLastCallRejectsAmbiguousSubjectIndex 0a7c93e93014a057
#[test]
fn test_data_first_or_last_call_rejects_ambiguous_subject_index() {
    run(|t| {
        t.parallel();

        let source = "
declare function operation<A>(first: A, second: A): A
declare function operation<A>(second: A): (first: A) => A

const result = operation(1, 2)
";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let call = find_variable_initializer_call_by_name(t, sf, "result");
        if tp.data_first_or_last_call(call).is_some() {
            t.fatal("expected an ambiguous subject index not to normalize");
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:838 logDerivedPipeableSignatureComparison 701eb555b83ffaab
// Rust: Go's nil parser and checker cannot occur (references); `c` is `tp.checker`.
fn log_derived_pipeable_signature_comparison(
    t: &T,
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) {
    t.helper();
    let Some(node) = node else {
        return;
    };

    let call = node.as_call_expression();
    if call.arguments().nodes().len() < 2 {
        return;
    }

    let resolved = tp.checker.get_resolved_signature_exported(node);
    let Some(resolved_declaration) = resolved.declaration() else {
        t.fatal("expected resolved data-first signature");
    };
    let actual_symbol = get_symbol_of_declaration(tp.checker, resolved_declaration);
    let resolved_param_count = resolved.parameters().len();
    let mut subject_indexes: Vec<i64> = vec![0];
    if call.arguments().nodes().len() > 1 {
        let last = call.arguments().nodes().len() as i64 - 1;
        let mut prefer_first = false;
        let params = resolved.parameters();
        if !params.is_empty() {
            prefer_first = is_likely_self_parameter(Some(params[0]));
        }
        if prefer_first {
            subject_indexes = vec![0, last];
        } else {
            subject_indexes = vec![last, 0];
        }
    }

    let resolved_return = tp.checker.get_return_type_of_signature_exported(resolved);
    t.log(sprintf!(
        "data-first resolved return type: %s",
        tp.checker.type_to_string_exported(resolved_return)
    ));
    for subject_index in subject_indexes {
        let Some(derived) =
            derive_pipeable_signature_from_data_first(tp.checker, Some(resolved), subject_index)
        else {
            t.log(sprintf!("subjectIndex=%d derived=nil", subject_index));
            continue;
        };
        let derived_return = tp.checker.get_return_type_of_signature_exported(derived);
        let inner_sigs =
            tp.checker.get_signatures_of_type_exported(derived_return, SignatureKind::Call);
        let mut inner_returns: Vec<String> = Vec::with_capacity(inner_sigs.len());
        for &sig in inner_sigs {
            let sig_return = tp.checker.get_return_type_of_signature_exported(sig);
            inner_returns.push(tp.checker.type_to_string_exported(sig_return));
        }
        t.log(sprintf!(
            "subjectIndex=%d derived return=%s typeArgs=%d innerReturns=%v",
            subject_index,
            tp.checker.type_to_string_exported(derived_return),
            derived.type_parameters().len() as i64,
            inner_returns
        ));

        // Rust: Go passes `resolvedParamCount-1`, which is never negative here (a resolved signature of a call
        // with two or more arguments); the count is unsigned in tsrs.
        let (_resolved_outer, candidates) =
            ets_shim::checker::get_resolved_signature_for_signature_help(
                node,
                resolved_param_count.saturating_sub(1),
                tp.checker,
            );
        for (i, &candidate) in candidates.iter().enumerate() {
            let Some(candidate_declaration) = candidate.declaration() else {
                continue;
            };
            let candidate_symbol = get_symbol_of_declaration(tp.checker, candidate_declaration);
            if candidate_symbol.is_none()
                || get_symbol_if_same_reference(tp.checker, actual_symbol, candidate_symbol)
                    .is_none()
            {
                continue;
            }
            let candidate_return = tp.checker.get_return_type_of_signature_exported(candidate);
            let returned =
                tp.checker.get_signatures_of_type_exported(candidate_return, SignatureKind::Call);
            let mut returned_arity: Vec<i64> = Vec::with_capacity(returned.len());
            for rs in returned {
                returned_arity.push(rs.parameters().len() as i64);
            }
            let derived_string = signature_string(tp.checker, Some(derived));
            let candidate_string = signature_string(tp.checker, Some(candidate));
            let candidate_return_string = tp.checker.type_to_string_exported(candidate_return);
            let matches = matches_pipeable_signature(
                tp,
                Some(resolved),
                Some(candidate),
                subject_index,
                None,
            );
            t.log(sprintf!(
                "derived=%s | candidate %d=%s | return=%s typeArgs=%d returnedArity=%v matches=%v",
                derived_string,
                i as i64,
                candidate_string,
                candidate_return_string,
                candidate.type_parameters().len() as i64,
                returned_arity,
                matches
            ));
        }
    }
}

// go test: internal/typeparser/data_first_signature_test.go:919 signatureString 8700f8d5e4823920
// Rust: Go's nil checker cannot occur (a reference).
fn signature_string(c: &mut ets_shim::checker::Checker, sig: Option<P<Signature>>) -> String {
    let Some(sig) = sig else {
        return "<nil>".to_string();
    };
    c.signature_to_string_ex(sig, None, TypeFormatFlags::WriteArrowStyleSignature, None)
}

// go test: internal/typeparser/data_first_signature_test.go:926 findFlowByNode 5bba96255b38ad8b
// Rust: Go's read-only view of the cached flows is the cached slice (`None` is a nil view).
fn find_flow_by_node<'a>(
    t: &T,
    sf: P<SourceFile>,
    flows: Option<&'a [PipingFlow]>,
    node: P<Node>,
) -> &'a PipingFlow {
    t.helper();
    for flow in flows.unwrap_or_default() {
        if flow.node == node {
            return flow;
        }
    }
    t.fatal(sprintf!("flow for node %q not found", node_text(Some(sf), Some(node))));
}

// go test: internal/typeparser/data_first_signature_test.go:937 assertSingleTransformation dbf445bf1976d331
fn assert_single_transformation(
    t: &T,
    sf: P<SourceFile>,
    flow: Option<&PipingFlow>,
    want_kind: TransformationKind,
    want_callee: &str,
    want_args: &[&str],
) {
    t.helper();
    let Some(flow) = flow else {
        t.fatal("flow is nil");
    };
    if flow.partial.transformations.len() != 1 {
        t.fatal(sprintf!(
            "transformation count = %d, want 1",
            flow.partial.transformations.len() as i64
        ));
    }
    let tr = &flow.partial.transformations[0];
    if tr.kind != want_kind {
        t.fatal(sprintf!("kind = %q, want %q", tr.kind.as_str(), want_kind.as_str()));
    }
    let got = node_text(Some(sf), Some(tr.callee)).trim().to_string();
    if got != want_callee {
        t.fatal(sprintf!("callee = %q, want %q", got, want_callee));
    }
    let args = tr.args.as_deref().unwrap_or_default();
    if args.len() != want_args.len() {
        t.fatal(sprintf!("arg count = %d, want %d", args.len() as i64, want_args.len() as i64));
    }
    for (i, &arg) in args.iter().enumerate() {
        let got = node_text(Some(sf), Some(arg)).trim().to_string();
        if got != want_args[i] {
            t.fatal(sprintf!("arg[%d] = %q, want %q", i as i64, got, want_args[i]));
        }
    }
}

// go test: internal/typeparser/data_first_signature_test.go:963 ptrTransformations c923db2080feb122
fn ptr_transformations(input: &[PipingFlowTransformation]) -> Vec<&PipingFlowTransformation> {
    let mut result = Vec::with_capacity(input.len());
    for transformation in input {
        result.push(transformation);
    }
    result
}

// go test: internal/typeparser/data_first_signature_test.go:971 stripWhitespace d9ea03cd56544e5c
fn strip_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join("")
}

// go test: internal/typeparser/data_first_signature_test.go:975 findVariableInitializerCallByName ffa704a956d7c8a1
// Rust: the call expression node (Go's `*ast.CallExpression`).
fn find_variable_initializer_call_by_name(t: &T, sf: P<SourceFile>, name: &str) -> P<Node> {
    t.helper();

    fn visit(node: P<Node>, name: &str, found: &mut Option<P<Node>>) {
        if found.is_some() {
            return;
        }
        if node.kind() == Kind::VariableDeclaration
            && let Some(decl_name) = node.name()
            && decl_name.kind() == Kind::Identifier
            && decl_name.text() == name
            && let Some(initializer) = node.initializer()
            && initializer.kind() == Kind::CallExpression
        {
            *found = Some(initializer);
            return;
        }
        node.for_each_child(&mut |child| {
            visit(child, name, found);
            false
        });
    }

    let mut found = None;
    visit(sf.as_node(), name, &mut found);
    let Some(found) = found else {
        t.fatal(ets_go::sprintf!("initializer call for variable %q not found", name));
    };
    found
}
