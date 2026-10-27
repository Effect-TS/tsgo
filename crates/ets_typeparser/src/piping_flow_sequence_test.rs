// Rust: the flows' steps carry a callee node and the predicates receive the parser (see
// piping_flow_shape_test.rs).

use ets_go::sprintf;

use crate::helpers_test::*;
use crate::piping_flow_shape_test::{zero_flow, zero_step};
use crate::*;

// go test: internal/typeparser/piping_flow_sequence_test.go:8 TestFindTransformationSequences 033ababa86ac6fda
#[test]
fn test_find_transformation_sequences() {
    use TransformationKind::{Call, DataFirst, Pipe};
    run(|t| {
        t.parallel();
        // name, steps, pattern, starts
        let tests: Vec<(&str, Vec<TransformationKind>, Vec<TransformationKind>, Vec<i64>)> = vec![
            ("nil flow", vec![], vec![], vec![]),
            ("empty flow", vec![], vec![Pipe], vec![]),
            ("empty pattern", vec![Pipe], vec![], vec![]),
            ("longer than flow", vec![Pipe], vec![Pipe, Pipe], vec![]),
            ("nonconsecutive", vec![Pipe, Call, DataFirst], vec![Pipe, DataFirst], vec![]),
            ("overlaps", vec![Pipe, Pipe, Pipe], vec![Pipe, Pipe], vec![0, 1]),
            (
                "multiple matches",
                vec![Call, Pipe, DataFirst, Call, Pipe, DataFirst],
                vec![Pipe, DataFirst],
                vec![1, 4],
            ),
            ("whole flow", vec![Pipe, Call], vec![Pipe, Call], vec![0]),
        ];
        for (name, steps, pattern, want_starts) in tests {
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, "");
                let mut tp = compiled.type_parser();
                let mut flow = None;
                if name != "nil flow" {
                    let mut f = zero_flow(sf.as_node());
                    for kind in steps {
                        f.partial.transformations.push(zero_step(sf.as_node(), kind));
                    }
                    flow = Some(f);
                }
                let predicates: Vec<
                    Box<dyn Fn(&mut TypeParser<'_>, &PipingFlowTransformation) -> bool>,
                > = pattern
                    .iter()
                    .map(|&kind| {
                        Box::new(move |_: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                            step.kind == kind
                        })
                            as Box<dyn Fn(&mut TypeParser<'_>, &PipingFlowTransformation) -> bool>
                    })
                    .collect();
                let predicates: Vec<TransformationPredicate<'_>> =
                    predicates.iter().map(|p| &**p as TransformationPredicate<'_>).collect();
                let matches =
                    PipingFlow::find_transformation_sequences(flow.as_ref(), &mut tp, &predicates);
                let mut starts: Vec<i64> = Vec::new();
                for m in matches.as_deref().unwrap_or_default() {
                    starts.push(m.start);
                    if m.transformations.len() != pattern.len() {
                        t.fatal("match must contain exactly the requested steps");
                    }
                    let original =
                        &flow.as_ref().expect("a match implies a flow").partial.transformations;
                    for i in 0..m.transformations.len() {
                        if !std::ptr::eq(
                            &raw const m.transformations[i],
                            &raw const original[m.start as usize + i],
                        ) {
                            t.fatal("matched steps must refer to the original flow");
                        }
                    }
                }
                if starts != want_starts {
                    t.fatal(sprintf!("got starts %v, want %v", starts, want_starts));
                }
                if want_starts.is_empty() && matches.is_some() {
                    t.fatal("no matches must return nil");
                }
            });
        }
        let (mut compiled, _) = compile_and_get_checker_and_source_file_internal(t, "");
        let mut tp = compiled.type_parser();
        let always = |_: &mut TypeParser<'_>, _: &PipingFlowTransformation| true;
        if PartialPipingFlow::find_transformation_sequences(None, &mut tp, &[&always]).is_some() {
            t.fatal("nil partial flow must return nil");
        }
    });
}
