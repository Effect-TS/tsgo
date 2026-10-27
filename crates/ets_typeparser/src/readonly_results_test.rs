// Ported from effect-tsgo's internal/typeparser/readonly_results_test.go (package typeparser_test: only the public
// API is used). Left out: the tail of TestReadonlyFlowConsumer that goes through `etsgoapi.NewTypeParser` (the
// public conversion and its cached arguments) and the final `assertFlow` after it: `ets_etsgoapi` depends on this
// crate and is not one of its dev-dependencies.

use ets_go::sprintf;
use ets_shim::ast::Node;
use ets_shim::core::{CompilerOptions, P, ScriptTarget, Tristate};
use ets_shim::vfs::vfstest::MapFile;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/readonly_results_test.go:21 TestReadonlyFlowConsumer 313d838e095c8484
// Rust: Go clears a slot of each copied slice (`nil` or the zero value); the copies are owned values here, so a
// slot is cleared by making the copy's elements optional. A cleared `PipingFlowTransformation` keeps its kind and
// callee, which have no zero value.
#[test]
fn test_readonly_flow_consumer() {
    run(|t| {
        t.parallel();
        const SOURCE: &str = "
declare const source: { pipe(...steps: Array<(n: number) => number>): number }
declare function add(n: number): (n: number) => number
export const result = source.pipe(add(1), add(2), add(1))
";
        let mut testfs = TestFS::default();
        testfs.insert("/test.ts".to_string(), MapFile::from(SOURCE));
        let mut compiled = new_compiled(
            testfs,
            CompilerOptions {
                target: ScriptTarget::ES_NEXT,
                strict: Tristate::True,
                ..Default::default()
            },
            vec!["/test.ts".to_string()],
        );
        let Some(sf) = compiled.program.get_source_file("/test.ts") else {
            t.fatal("source file missing");
        };
        let mut tp = compiled.type_parser();
        let flows = tp.piping_flows(sf, false);
        let flows_view = ets_readonly::new(flows.as_deref());
        if flows_view.len() != 1 {
            t.fatal(sprintf!("got %d flows, want 1", flows_view.len()));
        }
        let flow = flows_view.at(0);
        let text = |node: Option<P<Node>>| -> String {
            let Some(node) = node else {
                return "<nil>".to_string();
            };
            let pos = node.pos().source_offset().unwrap() as usize;
            let end = node.end().source_offset().unwrap() as usize;
            sf.text()[pos..end].trim().to_string()
        };
        let assert_flow = || {
            t.helper();
            let got = text(flow.subject().node());
            if got != "source" {
                t.fatal(sprintf!("subject = %q, want source", got));
            }
            let mut args: Vec<String> = Vec::new();
            for (_, step) in flow.transformations().all() {
                if step.kind() != TransformationKind::Pipeable
                    || text(Some(step.callee())) != "add"
                    || step.args().len() != 1
                {
                    t.fatal("pipe transformation changed");
                }
                args.push(text(Some(*step.args().at(0))));
            }
            if args != ["1", "2", "1"] {
                t.fatal(sprintf!("arguments = %v, want [1 2 1]", args));
            }
        };
        assert_flow();

        let mut owned_flows: Vec<Option<PipingFlow>> =
            flows_view.copy().unwrap_or_default().into_iter().map(Some).collect();
        owned_flows[0] = None;
        let mut owned_steps: Vec<Option<PipingFlowTransformation>> =
            flow.transformations().copy().unwrap_or_default().into_iter().map(Some).collect();
        owned_steps[0] = None;
        let mut owned_args: Vec<Option<P<Node>>> = flow
            .transformations()
            .at(0)
            .args()
            .copy()
            .unwrap_or_default()
            .into_iter()
            .map(Some)
            .collect();
        owned_args[0] = None;
        let mut partial = flow.partial();
        if partial.transformations().len() != 3 {
            t.fatal("partial flow lost transformations");
        }
        partial = PartialPipingFlow::default();
        if partial.transformations().len() != 0 {
            t.fatal("local partial was not replaced");
        }

        let subject = |_: &mut TypeParser<'_>, value: &PipingFlowSubject| -> bool {
            let mut value = value.clone();
            let matched = text(value.node()) == "source";
            value = PipingFlowSubject::default();
            if value.node().is_some() {
                t.fatal("subject copy was not replaced");
            }
            matched
        };
        let step = |want: &'static str| {
            move |_: &mut TypeParser<'_>, value: &PipingFlowTransformation| -> bool {
                let mut value = value.clone();
                let matched = value.args().len() == 1 && text(Some(*value.args().at(0))) == want;
                value = PipingFlowTransformation {
                    kind: value.kind,
                    callee: value.callee,
                    type_arguments: None,
                    args: None,
                    out_type: None,
                };
                if value.args().len() != 0 {
                    t.fatal("step copy was not replaced");
                }
                matched
            }
        };
        let (step1, step2) = (step("1"), step("2"));
        if !PipingFlow::matches_prefix(Some(flow), &mut tp, &subject, &[&step1])
            || !PipingFlow::matches_exactly(
                Some(flow),
                &mut tp,
                &subject,
                &[&step1, &step2, &step1],
            )
        {
            t.fatal("mutating predicate copies changed matching");
        }
        let matches = PipingFlow::find_transformation_sequences(Some(flow), &mut tp, &[&step1]);
        let mut starts: Vec<i64> = Vec::new();
        for m in matches.as_deref().unwrap_or_default() {
            starts.push(m.start());
            let mut owned: Vec<Option<PipingFlowTransformation>> =
                m.transformations().copy().unwrap_or_default().into_iter().map(Some).collect();
            owned[0] = None;
        }
        if starts != [0, 2] {
            t.fatal(sprintf!("sequence starts = %v, want [0 2]", starts));
        }
        let again = tp.piping_flows(sf, false);
        if !std::ptr::eq(ets_readonly::new(again.as_deref()).at(0), flow) {
            t.fatal("cached flow identity changed");
        }
        assert_flow();
    });
}
