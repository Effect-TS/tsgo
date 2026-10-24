// Rust: Go's three distinct `&checker.Type{}` values are the checker's string, number and boolean types of an
// empty program, and its steps carry a callee node (see piping_flow_shape_test.rs).

use crate::helpers_test::*;
use crate::piping_flow_shape_test::zero_step;
use crate::*;

// go test: internal/typeparser/piping_flow_input_type_test.go:8 TestTransformationInputType
#[test]
fn test_transformation_input_type() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, "");
        let mut tp = compiled.type_parser();
        let (subject, intermediate, output) =
            (tp.checker.string_type, tp.checker.number_type, tp.checker.boolean_type);
        let mut first = zero_step(sf.as_node(), TransformationKind::Pipe);
        first.out_type = Some(resolved_lazy(Some(intermediate)));
        let mut second = zero_step(sf.as_node(), TransformationKind::Pipe);
        second.out_type = Some(resolved_lazy(Some(output)));
        let mut flow = PartialPipingFlow {
            subject: PipingFlowSubject { node: None, out_type: Some(resolved_lazy(Some(subject))) },
            transformations: vec![first, second],
        };
        if PartialPipingFlow::transformation_input_type(Some(&flow), &mut tp, 0) != Some(subject) {
            t.fatal("first step must consume the subject type");
        }
        if PartialPipingFlow::transformation_input_type(Some(&flow), &mut tp, 1)
            != Some(intermediate)
        {
            t.fatal("later steps must consume the preceding output type");
        }
        for index in [-1, 2] {
            if PartialPipingFlow::transformation_input_type(Some(&flow), &mut tp, index).is_some() {
                t.fatal("out-of-range step must have no input type");
            }
        }
        flow.transformations[0].out_type = None;
        if PartialPipingFlow::transformation_input_type(Some(&flow), &mut tp, 1).is_some() {
            t.fatal("missing output type must remain unknown");
        }
        if PipingFlow::transformation_input_type(None, &mut tp, 0).is_some() {
            t.fatal("nil complete flow must have no input type");
        }
        if PartialPipingFlow::transformation_input_type(None, &mut tp, 0).is_some() {
            t.fatal("nil flow must have no input type");
        }
    });
}
