// Rust: Go builds flows from zero `PipingFlowTransformation{kind: k}` values; a Rust step needs a callee node, so
// the steps' callee (and the flows' node) is the source file node of an empty program (`zero_step`), which the
// matchers never read. The predicates receive the parser (see `SubjectPredicate`), so the tests run on a parser
// over that program.

use std::sync::Arc;

use ets_go::sprintf;
use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

/// Rust-only: Go's `PipingFlowTransformation{kind: kind}` (see the file comment).
pub(crate) fn zero_step(node: P<Node>, kind: TransformationKind) -> PipingFlowTransformation {
    PipingFlowTransformation {
        kind,
        callee: node,
        type_arguments: None,
        args: None,
        out_type: None,
    }
}

/// Rust-only: Go's `PipingFlow{}` (see the file comment).
pub(crate) fn zero_flow(node: P<Node>) -> PipingFlow {
    PipingFlow { node, partial: PartialPipingFlow::default() }
}

// go test: internal/typeparser/piping_flow_shape_test.go:10 TestPipingFlowMatchesShape 102d82be4a667622
#[test]
fn test_piping_flow_matches_shape() {
    use TransformationKind::{Call, Pipe};
    run(|t| {
        t.parallel();
        // name, subjectMatches, steps, pattern, prefix, exact
        let tests: Vec<(&str, bool, Vec<TransformationKind>, Vec<TransformationKind>, bool, bool)> = vec![
            ("subject only", true, vec![], vec![], true, true),
            ("subject mismatch", false, vec![], vec![], false, false),
            ("subject only with extra steps", true, vec![Pipe], vec![], true, false),
            ("whole flow", true, vec![Pipe, Call], vec![Pipe, Call], true, true),
            ("prefix with extra steps", true, vec![Pipe, Call], vec![Pipe], true, false),
            ("too few steps", true, vec![Pipe], vec![Pipe, Call], false, false),
            ("empty flow with expected step", true, vec![], vec![Pipe], false, false),
            ("later step mismatch", true, vec![Pipe, Call], vec![Pipe, Pipe], false, false),
            ("matching sequence is not at head", true, vec![Call, Pipe], vec![Pipe], false, false),
            ("steps match but subject does not", false, vec![Pipe], vec![Pipe], false, false),
        ];
        for (name, subject_matches, steps, pattern, prefix, exact) in tests {
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, "");
                let mut tp = compiled.type_parser();
                let mut flow = zero_flow(sf.as_node());
                for kind in steps {
                    flow.partial.transformations.push(zero_step(sf.as_node(), kind));
                }
                let subject = |_: &mut TypeParser<'_>, _: &PipingFlowSubject| subject_matches;
                let predicates: Vec<
                    Box<dyn Fn(&mut TypeParser<'_>, &PipingFlowTransformation) -> bool>,
                > = pattern
                    .into_iter()
                    .map(|kind| {
                        Box::new(move |_: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                            step.kind == kind
                        })
                            as Box<dyn Fn(&mut TypeParser<'_>, &PipingFlowTransformation) -> bool>
                    })
                    .collect();
                let predicates: Vec<TransformationPredicate<'_>> =
                    predicates.iter().map(|p| &**p as TransformationPredicate<'_>).collect();
                let got = PipingFlow::matches_prefix(Some(&flow), &mut tp, &subject, &predicates);
                if got != prefix {
                    t.fatal(sprintf!("MatchesPrefix = %v, want %v", got, prefix));
                }
                let got = PipingFlow::matches_exactly(Some(&flow), &mut tp, &subject, &predicates);
                if got != exact {
                    t.fatal(sprintf!("MatchesExactly = %v, want %v", got, exact));
                }
            });
        }
    });
}

// go test: internal/typeparser/piping_flow_shape_test.go:54 TestPipingFlowMatchesShapeNil 3f7f275f44a1ea83
#[test]
fn test_piping_flow_matches_shape_nil() {
    run(|t| {
        t.parallel();
        let (mut compiled, _) = compile_and_get_checker_and_source_file_internal(t, "");
        let mut tp = compiled.type_parser();
        let subject = |_: &mut TypeParser<'_>, _: &PipingFlowSubject| -> bool {
            t.fatal("nil flow must not evaluate predicates");
        };
        if PartialPipingFlow::matches_prefix(None, &mut tp, &subject, &[])
            || PartialPipingFlow::matches_exactly(None, &mut tp, &subject, &[])
            || PipingFlow::matches_prefix(None, &mut tp, &subject, &[])
            || PipingFlow::matches_exactly(None, &mut tp, &subject, &[])
        {
            t.fatal("nil flows must not match");
        }
    });
}

// go test: internal/typeparser/piping_flow_shape_test.go:64 TestParsePipeCall_ConstantAlias d2c55e5eead58d32
#[test]
fn test_parse_pipe_call_constant_alias() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_version_internal(
                t,
                EFFECT_V4,
                r#"
import { pipe } from "effect"

const p = pipe
export const value = p(1)
"#,
            );
        let mut tp = compiled.type_parser();

        fn visit(
            tp: &mut TypeParser<'_>,
            node: P<Node>,
            parsed: &mut Option<Arc<ParsedPipeCallResult>>,
        ) {
            if parsed.is_some() {
                return;
            }
            if let Some(result) = tp.parse_pipe_call(node) {
                *parsed = Some(result);
                return;
            }
            node.for_each_child(&mut |child| {
                visit(tp, child, parsed);
                false
            });
        }
        let mut parsed = None;
        visit(&mut tp, sf.as_node(), &mut parsed);

        if !parsed
            .is_some_and(|parsed| parsed.kind == TransformationKind::Pipe && parsed.args.is_empty())
        {
            t.fatal("expected a constant alias of pipe to parse as a standalone pipe call");
        }
    });
}
