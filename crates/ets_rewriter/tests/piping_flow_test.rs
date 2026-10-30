use std::sync::Arc;

use ets_bundledeffect::{EffectVersion, TestFS};
use ets_shim::ast::SourceFile;
use ets_shim::compiler::{CheckerHandle, ProgramOptions};
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, P, ScriptTarget, TextChange, Tristate,
};
use ets_shim::ls::{change, lsconv, lsutil};
use ets_shim::lsproto;
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use ets_typeparser::{PipingFlow, TransformationKind};

// go test: internal/rewriter/piping_flow_test.go:24 TestRemoveTrailingPipingFlowTransformation 3018df64867d41f2
// Rust: Go's `(*rewriter.Tracker)(nil)` receiver check is left out (a Rust `&mut Tracker` is never nil). The
// subtests run in sequence; each program is leaked, as the test process ends right after.
#[test]
fn test_remove_trailing_piping_flow_transformation() {
    ets_go::testing::on_checker_stack(|| {
        ets_go::testing::run(|t| {
            const PREFIX: &str = r#"import { Effect, pipe } from "effect"
declare const source: Effect.Effect<number>
declare function unary(self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number, self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number): (self: Effect.Effect<number>) => Effect.Effect<number>
export const value = "#;

            struct Case {
                name: &'static str,
                source: &'static str,
                expected: &'static str,
                kind: Option<TransformationKind>,
                remove: bool,
                deletions_only: bool,
            }
            const fn case(
                name: &'static str,
                source: &'static str,
                expected: &'static str,
                kind: Option<TransformationKind>,
                remove: bool,
                deletions_only: bool,
            ) -> Case {
                Case { name, source, expected, kind, remove, deletions_only }
            }
            use TransformationKind::*;
            let tests = [
                case("unary call", "unary(source)", "source", Some(Call), true, false),
                case(
                    "data last call",
                    "dataLast(0, source.pipe(Effect.map((n) => n + 1)))",
                    "source.pipe(Effect.map((n) => n + 1))",
                    Some(DataLast),
                    true,
                    false,
                ),
                case(
                    "parenthesized curried call",
                    "(Effect.as(0))(source.pipe(Effect.map((n) => n + 1)))",
                    "source.pipe(Effect.map((n) => n + 1))",
                    Some(Call),
                    true,
                    false,
                ),
                case(
                    "block comment after comma",
                    "source.pipe(Effect.as(1), /* keep */ \t Effect.as(0))",
                    "source.pipe(Effect.as(1) /* keep */)",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "block comment before comma",
                    "source.pipe(Effect.as(1) /* keep */ \t, \t Effect.as(0))",
                    "source.pipe(Effect.as(1) /* keep */)",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "comments on both sides",
                    "source.pipe(Effect.as(1) /* before */ \t, /* after */ \t Effect.as(0))",
                    "source.pipe(Effect.as(1) /* before */ \t /* after */)",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "line comment before comma",
                    "source.pipe(Effect.as(1) // keep\n \t, \tEffect.as(0))",
                    "source.pipe(Effect.as(1) // keep\n)",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "comments and carriage return",
                    "pipe(source, Effect.as(1), // keep 😀\r\n\t Effect.as(0))",
                    "pipe(source, Effect.as(1) // keep 😀\r\n)",
                    Some(Pipe),
                    true,
                    true,
                ),
                case(
                    "nonbreaking space",
                    "source.pipe(Effect.as(1), /* keep */\u{00a0} \tEffect.as(0))",
                    "source.pipe(Effect.as(1) /* keep */\u{00a0})",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "trailing comma",
                    "source.pipe(Effect.as(1), /* keep */ Effect.as(0),)",
                    "source.pipe(Effect.as(1) /* keep */,)",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "source spelling",
                    r"source.pipe(Effect.as('\u0061'), Effect.as('discard'))",
                    r"source.pipe(Effect.as('\u0061'))",
                    Some(Pipeable),
                    true,
                    true,
                ),
                case(
                    "direct source spelling",
                    r"Effect.as(Effect.succeed(/* keep */ '\u0061'), 0)",
                    r"Effect.succeed(/* keep */ '\u0061')",
                    Some(DataFirst),
                    true,
                    false,
                ),
                case(
                    "missing separator",
                    "source.pipe(Effect.as(1) /* keep */ Effect.as(0))",
                    "",
                    Some(Pipeable),
                    false,
                    false,
                ),
                case("optional unary call", "unary?.(source)", "", Some(Call), false, false),
                case(
                    "optional curried factory",
                    "(Effect.as?.(0))(source)",
                    "",
                    Some(Call),
                    false,
                    false,
                ),
                case(
                    "optional method pipe",
                    "source?.pipe(Effect.as(0))",
                    "",
                    Some(Pipeable),
                    false,
                    false,
                ),
                case(
                    "optional pipe application",
                    "source.pipe?.(Effect.as(0))",
                    "",
                    Some(Pipeable),
                    false,
                    false,
                ),
                case(
                    "optional pipe operator",
                    "source.pipe(Effect.as(1), Effect.as?.(0))",
                    "",
                    Some(Pipeable),
                    false,
                    false,
                ),
                case(
                    "unsupported Effect fn",
                    "Effect.fn(function*() { return 1 }, Effect.as(0))",
                    "",
                    Some(EffectFn),
                    false,
                    false,
                ),
                case("empty flow", "source", "", None, false, false),
            ];

            for test in tests {
                t.run(test.name, |t| {
                    let source = format!("{PREFIX}{}", test.source);
                    let PipingFlowRewriteFixture { sf, flow, mut tracker, converters, .. } =
                        new_piping_flow_rewrite_fixture(t, &source);
                    if let Some(kind) = test.kind {
                        let transformations = flow.transformations();
                        if transformations.len() == 0
                            || transformations.at(transformations.len() - 1).kind() != kind
                        {
                            t.fatal(format!(
                                "expected terminal transformation kind {}",
                                kind.as_str()
                            ));
                        }
                    }
                    let got =
                        tracker.remove_trailing_piping_flow_transformation(Some(sf), Some(&flow));
                    if got != test.remove {
                        t.fatal(format!("removal = {got}, want {}", test.remove));
                    }
                    if tracker.remove_trailing_piping_flow_transformation(Some(sf), None)
                        || tracker.remove_trailing_piping_flow_transformation(None, Some(&flow))
                    {
                        t.fatal("nil inputs must not remove transformations");
                    }
                    let edits =
                        tracker.get_changes().shift_remove(sf.file_name()).unwrap_or_default();
                    if !test.remove && !edits.is_empty() {
                        t.fatal(format!("unsupported removal produced {} edits", edits.len()));
                    }
                    let mut changes = Vec::new();
                    for edit in &edits {
                        if test.deletions_only && !edit.new_text.is_empty() {
                            t.fatal(format!("pipe removal inserted text {:?}", edit.new_text));
                        }
                        let span = converters.from_lsp_range_to_original(&sf, edit.range);
                        changes
                            .push(TextChange { text_range: span, new_text: edit.new_text.clone() });
                    }
                    let mut expected = source.clone();
                    if test.remove {
                        expected = format!("{PREFIX}{}", test.expected);
                    }
                    let got = ets_shim::core::apply_bulk_edits(&source, &changes);
                    if got != expected {
                        t.fatal(format!("replacement = {got:?}, want {expected:?}"));
                    }
                });
            }
        });
    });
}

// go test: internal/rewriter/piping_flow_test.go:147 TestReplacePipingFlowPrefix 15a6f1b2e43982be
// Rust: the subtests run in sequence; each program is leaked, as the test process ends right after.
#[test]
fn test_replace_piping_flow_prefix() {
    ets_go::testing::on_checker_stack(|| {
        ets_go::testing::run(|t| {
            const PREFIX: &str = r#"import { Effect, pipe } from "effect"
declare const source: Effect.Effect<number>
declare const other: Effect.Effect<number>
declare function unary(self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number, self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number): (self: Effect.Effect<number>) => Effect.Effect<number>
declare function withOptions(self: Effect.Effect<number>, options?: { readonly n: number }): Effect.Effect<number>
export const value = "#;

            use TransformationKind::*;
            let tests: [(&str, &str, &str, TransformationKind, i64, bool); 9] = [
                ("unary call", "unary(source)", "other", Call, 1, true),
                ("data-first call", "Effect.as(source, 1)", "other", DataFirst, 1, true),
                ("data-last call", "dataLast(1, source)", "other", DataLast, 1, true),
                (
                    "call with optionals",
                    "withOptions(source, { n: 1 })",
                    "other",
                    CallWithOptionals,
                    1,
                    true,
                ),
                (
                    "pipe prefix",
                    "pipe(source, Effect.as(1), Effect.as(2))",
                    "pipe(other, Effect.as(2))",
                    Pipe,
                    1,
                    true,
                ),
                ("whole pipe", "pipe(source, Effect.as(1), Effect.as(2))", "other", Pipe, 2, true),
                (
                    "method pipe prefix",
                    "source.pipe(Effect.as(1), Effect.as(2))",
                    "other.pipe(Effect.as(2))",
                    Pipeable,
                    1,
                    true,
                ),
                (
                    "unsupported Effect fn",
                    "Effect.fn(function*() { return 1 }, Effect.as(0))",
                    "",
                    EffectFn,
                    1,
                    false,
                ),
                ("count out of range", "unary(source)", "", Call, 2, false),
            ];

            for (name, test_source, test_expected, kind, count, replace) in tests {
                t.run(name, |t| {
                    let source = format!("{PREFIX}{test_source}");
                    let mut fixture = new_piping_flow_rewrite_fixture(t, &source);
                    let first = fixture.flow.transformations().at(0).kind();
                    if first != kind {
                        t.fatal(format!(
                            "expected first transformation kind {}, got {}",
                            kind.as_str(),
                            first.as_str()
                        ));
                    }
                    let replacement = fixture.tracker.new_identifier("other");
                    let got = fixture.tracker.replace_piping_flow_prefix(
                        Some(fixture.sf),
                        Some(&fixture.flow),
                        count,
                        Some(replacement),
                    );
                    if got != replace {
                        t.fatal(format!("replacement = {got}, want {replace}"));
                    }
                    let mut expected = source;
                    if replace {
                        expected = format!("{PREFIX}{test_expected}");
                    }
                    let got = fixture.apply();
                    if got != expected {
                        t.fatal(format!("result = {got:?}, want {expected:?}"));
                    }
                });
            }
        });
    });
}

// go test: internal/rewriter/piping_flow_test.go:196 pipingFlowRewriteFixture 7b4405e4a6340191
// pipingFlowRewriteFixture holds the longest piping flow at the initializer of a
// source's last declaration, and a tracker to rewrite it.
//
// Rust: the checker handle is kept for the fixture's lifetime (Go releases it in `t.Cleanup`).
struct PipingFlowRewriteFixture {
    source: String,
    sf: P<SourceFile>,
    flow: PipingFlow,
    tracker: ets_rewriter::Tracker,
    converters: Arc<lsconv::Converters>,
    _checker: CheckerHandle,
}

// go test: internal/rewriter/piping_flow_test.go:204 newPipingFlowRewriteFixture f4d8a84b1829431a
fn new_piping_flow_rewrite_fixture(
    t: &ets_go::testing::T,
    source: &str,
) -> PipingFlowRewriteFixture {
    let mut files = TestFS::default();
    files.insert("/test.ts".to_string(), MapFile::from(source.to_string()));
    if let Err(err) = ets_bundledeffect::mount_effect(EffectVersion::V4, &mut files) {
        t.fatal(err.to_string());
    }
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(files, true)));
    let options = CompilerOptions {
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        strict: Tristate::True,
        skip_lib_check: Tristate::True,
        ..Default::default()
    };
    let config = ets_shim::tsoptions::new_parsed_command_line(
        P::new(options.clone()),
        vec!["/test.ts".to_string()],
        Vec::new(),
        ComparePathsOptions { use_case_sensitive_file_names: true, current_directory: "/".into() },
    );
    let host =
        ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None);
    let mut program_options = ProgramOptions::new(P::new(config), host);
    program_options.single_threaded = Tristate::True;
    let program = ets_shim::compiler::new_program(program_options);
    let ctx = Context::background();
    let mut c = program.get_type_checker(&ctx);
    let sf = program.get_source_file("/test.ts").expect("the test file");
    let statements = sf.statements().nodes();
    let declaration = statements[statements.len() - 1]
        .as_variable_statement()
        .declaration_list()
        .as_variable_declaration_list()
        .declarations()
        .nodes()[0]
        .as_variable_declaration();
    let flow = ets_typeparser::new_type_parser(program, &mut c)
        .longest_piping_flow_at(declaration.initializer(), true);
    let Some(flow) = flow else {
        t.fatal("expected a parsed flow");
    };
    let converters =
        lsconv::new_converters(lsproto::PositionEncodingKind::UTF16, move |_: &str| {
            Some(lsconv::compute_lsp_line_starts(sf.text()))
        });
    let tracker = ets_rewriter::new_tracker(change::new_tracker(
        &ctx,
        &options,
        lsutil::get_default_format_code_settings(),
        Arc::clone(&converters),
    ));
    PipingFlowRewriteFixture {
        source: source.to_string(),
        sf,
        flow,
        tracker,
        converters,
        _checker: c,
    }
}

impl PipingFlowRewriteFixture {
    // go test: internal/rewriter/piping_flow_test.go:237 pipingFlowRewriteFixture.apply 952b31a2fbe263eb
    // apply returns the source with the tracker's edits applied.
    //
    // Rust: Go's `t` argument is only used for `t.Helper()`, so it is dropped.
    fn apply(&mut self) -> String {
        let mut changes = Vec::new();
        for edit in self.tracker.get_changes().shift_remove(self.sf.file_name()).unwrap_or_default()
        {
            let span = self.converters.from_lsp_range_to_original(&self.sf, edit.range);
            changes.push(TextChange { text_range: span, new_text: edit.new_text.clone() });
        }
        ets_shim::core::apply_bulk_edits(&self.source, &changes)
    }
}
