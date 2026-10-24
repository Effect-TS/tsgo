use std::sync::Arc;

use ets_shim::ast::SourceFileParseOptions;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::context::Context as GoContext;
use ets_shim::core::tspath::{self, ComparePathsOptions};
use ets_shim::core::{CompilerOptions, P, ScriptKind, TextRange, Tristate};
use ets_shim::lsproto;
use ets_shim::vfs::{FS, bundled, vfstest};

use crate::*;

// go test: internal/fixable/context_test.go:13 TestStandaloneRangeConversionUsesLSPLineSemantics
// Rust: Go builds a `Context` literal with only `SourceFile` and `converters` (the zero values of the rest are
// never read); a Rust `Context` needs a type parser, so the test makes one over an empty program. The subtests run
// in sequence.
#[test]
fn test_standalone_range_conversion_uses_lsp_line_semantics() {
    let source_file = ets_shim::parser::parse_source_file(
        SourceFileParseOptions {
            file_name: "/source.ts".to_string(),
            path: tspath::to_path("/source.ts", "", true),
            ..Default::default()
        },
        "😀x\u{2028}y",
        ScriptKind::TS,
    );

    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(
        [("/empty.ts".to_string(), vfstest::MapFile::from(""))],
        true,
    )));
    let host =
        ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None);
    let config = ets_shim::tsoptions::new_parsed_command_line(
        P::new(CompilerOptions { no_lib: Tristate::True, ..Default::default() }),
        vec!["/empty.ts".to_string()],
        Vec::new(),
        ComparePathsOptions { use_case_sensitive_file_names: true, current_directory: "/".into() },
    );
    let mut options = ProgramOptions::new(P::new(config), host);
    options.single_threaded = Tristate::True;
    let program = ets_shim::compiler::new_program(options);
    let mut checker = program.get_type_checker(&GoContext::background());

    let ctx = Context {
        source_file,
        span: TextRange::new(0, 0),
        error_code: 0,
        options: None,
        program,
        type_parser: ets_typeparser::new_type_parser(program, &mut checker),
        context: GoContext::background(),
        format_options: ets_shim::ls::lsutil::get_default_format_code_settings(),
        converters: new_standalone_converters(source_file),
    };

    struct Case {
        name: &'static str,
        start: u32,
        end: u32,
        want: &'static str,
    }
    for test in [
        Case { name: "after non-BMP character", start: 2, end: 3, want: "x" },
        Case { name: "after Unicode line separator", start: 4, end: 5, want: "y" },
    ] {
        let rng = ctx.lsp_range_to_text_range(lsproto::Range {
            start: lsproto::Position { line: 0, character: test.start },
            end: lsproto::Position { line: 0, character: test.end },
        });
        let pos = rng.pos().expect_source("a converted position") as usize;
        let end = rng.end().expect_source("a converted position") as usize;
        let got = &source_file.text()[pos..end];
        assert!(got == test.want, "{}: unexpected converted range text: {got:?}", test.name);
        let position = ctx.byte_pos_to_lsp_position(pos as u32);
        assert!(
            position.line == 0 && position.character == test.start,
            "{}: unexpected round-trip position: {position:?}",
            test.name
        );
    }
}
