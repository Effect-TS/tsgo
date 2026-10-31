// Ported from effect-tsgo's internal/typeparser/data_first_signature_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::SignatureKind;
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/data_first_signature_test.go:14 TestReconstructPipingFlow_NormalizedCalls 5592cd19a1406541
#[test]
fn test_reconstruct_piping_flow_normalized_calls() {
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
declare const signal: AbortSignal

export const provided = Effect.provide(program, MyService.Default, { local: true })
export const live = Layer.succeed(MyService, make)
export const run = Effect.runPromise(program, { signal })
"#;

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let flows = tp.piping_flows(sf, false);
        let cases: [(&str, TransformationKind, &str); 3] = [
            (
                "provided",
                TransformationKind::DataFirst,
                "Effect.provide(MyService.Default, { local: true })(program)",
            ),
            ("live", TransformationKind::DataLast, "Layer.succeed(MyService)(make)"),
            (
                "run",
                TransformationKind::CallWithOptionals,
                "Effect.runPromise(program, { signal })",
            ),
        ];
        for (name, kind, want) in cases {
            let flow = find_flow_by_node(
                t,
                sf,
                flows.as_deref(),
                find_variable_initializer_call_by_name(t, sf, name),
            );
            if flow.partial.transformations.len() != 1
                || flow.partial.transformations[0].kind != kind
            {
                t.error(sprintf!("%s: expected a single %s transformation", name, kind.as_str()));
                continue;
            }
            let got = strip_whitespace(&reconstruct_piping_flow(
                sf,
                Some(&flow.partial.subject),
                &ptr_transformations(&flow.partial.transformations),
            ));
            if got != strip_whitespace(want) {
                t.error(sprintf!("%s reconstructed flow = %q, want %q", name, got, want));
            }
        }
    });
}

// go test: internal/typeparser/data_first_signature_test.go:59 TestPipingFlows_TypeArguments 61662b53d3dd6787
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

// go test: internal/typeparser/data_first_signature_test.go:106 TestPipingFlows_DecomposesOnlyCurriedPipeableOverloads 5c5976e1c9f2ddd2
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

// go test: internal/typeparser/data_first_signature_test.go:141 TestPipingFlows_TransformationOccurrencesAreUnique 8f7bcdcc6824aaf2
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

// go test: internal/typeparser/data_first_signature_test.go:185 TestLongestPipingFlowAt_ReturnsLongestNestedFlow e9363406bdee1ac2
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

// go test: internal/typeparser/data_first_signature_test.go:273 transformationCallees 67a545e417da150a
fn transformation_callees(sf: P<SourceFile>, flow: &PipingFlow) -> Vec<String> {
    let mut result = Vec::with_capacity(flow.partial.transformations.len());
    for transformation in &flow.partial.transformations {
        result.push(node_text(Some(sf), Some(transformation.callee)).trim().to_string());
    }
    result
}

// go test: internal/typeparser/data_first_signature_test.go:281 TestPipeableSignatureShapeWithoutExplicitDeclarations 845260f6589de5ca
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
        if !matches_pipeable_signature_exported(
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

// go test: internal/typeparser/data_first_signature_test.go:318 TestPipeableSignatureShapeDoesNotInstantiateTypes 0d1054291f598b6e
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
        if !pipeable_signature_shapes_match(&mut tp, Some(resolved), Some(pipeable), 0, false) {
            t.fatal("expected signatures to be alpha-equivalent");
        }
        if !matches_pipeable_signature_exported(&mut tp, Some(resolved), Some(pipeable), 0, None) {
            t.fatal("expected full matcher to recognize the signatures");
        }
        let before = tp.checker.total_instantiation_count;
        for _ in 0..100 {
            if !compare_pipeable_signature_types(
                &mut tp,
                raw_signature(Some(resolved)),
                raw_signature(Some(pipeable)),
                0,
                false,
            ) {
                t.fatal("expected repeated raw type comparison to match");
            }
            if !pipeable_signature_shapes_match(&mut tp, Some(resolved), Some(pipeable), 0, false) {
                t.fatal("expected cached signature comparison to match");
            }
            if !matches_pipeable_signature_exported(
                &mut tp,
                Some(resolved),
                Some(pipeable),
                0,
                None,
            ) {
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

// go test: internal/typeparser/data_first_signature_test.go:368 TestPipeableSignatureShapeFailsClosedAtDepthBudget fd013309b1aaec27
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

// go test: internal/typeparser/data_first_signature_test.go:390 findFlowByNode 5bba96255b38ad8b
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

// go test: internal/typeparser/data_first_signature_test.go:401 assertSingleTransformation dbf445bf1976d331
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

// go test: internal/typeparser/data_first_signature_test.go:427 ptrTransformations c923db2080feb122
fn ptr_transformations(input: &[PipingFlowTransformation]) -> Vec<&PipingFlowTransformation> {
    let mut result = Vec::with_capacity(input.len());
    for transformation in input {
        result.push(transformation);
    }
    result
}

// go test: internal/typeparser/data_first_signature_test.go:435 stripWhitespace d9ea03cd56544e5c
fn strip_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join("")
}

// go test: internal/typeparser/data_first_signature_test.go:439 findVariableInitializerCallByName ffa704a956d7c8a1
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
