use std::fmt::Write;
use std::path::PathBuf;

use ets_etscore::EffectPluginOptions;
use ets_go::testing::T;
use ets_layergraph::ExtractLayerGraphOptions;
use ets_shim::ast::{Diagnostic, Kind, ModifierFlags, Node, SourceFile};
use ets_shim::checker::Checker;
use ets_shim::checker::types::TypeFormatFlags;
use ets_shim::compiler::Program;
use ets_shim::core::P;
use ets_shim::diagnostics::Category;

use crate::*;

// go test: internal/effecttest/baseline.go:26 TestDataPath 042a515cb6c2e99c
// TestDataPath returns the path to our testdata directory.
pub fn test_data_path() -> PathBuf {
    ets_bundledeffect::effect_ts_go_root_path().join("testdata")
}

// go test: internal/effecttest/baseline.go:31 BaselineLocalPath 000211a0e181e916
// BaselineLocalPath returns the path to write local baselines.
//
// Rust: `local-rs` instead of Go's `local`, so the two runners never overwrite each other's actual output (both
// directories are git-ignored).
pub fn baseline_local_path(subfolder: &str) -> PathBuf {
    test_data_path().join("baselines").join("local-rs").join(subfolder)
}

// go test: internal/effecttest/baseline.go:36 BaselineReferencePath 1cfe123f94b3dba0
// BaselineReferencePath returns the path to reference baselines.
pub fn baseline_reference_path(subfolder: &str) -> PathBuf {
    test_data_path().join("baselines").join("reference").join(subfolder)
}

// go test: internal/effecttest/baseline.go:42 DoEffectErrorBaseline bb32a9b1ee3610b8
// DoEffectErrorBaseline generates an error baseline for Effect tests.
// This writes to our testdata directory instead of typescript-go's.
pub fn do_effect_error_baseline(
    t: &T,
    baseline_name: &str,
    all_files: &[TestFile],
    diagnostics: &[P<Diagnostic>],
    _pretty: bool,
    subfolder: &str,
    effect_version: &str,
) {
    // Generate the baseline content
    let baseline_content = generate_error_baseline(effect_version, all_files, diagnostics);

    // Run baseline comparison
    run_effect_baseline(t, &format!("{baseline_name}.errors.txt"), &baseline_content, subfolder);
}

// go test: internal/effecttest/baseline.go:59 nonWhitespace 0138c44ce6931657
// nonWhitespace matches non-whitespace characters for preserving indentation in squiggles.
//
// Rust: Go's `nonWhitespace.ReplaceAllString(s, " ")` over a byte prefix of a line (which Go may cut inside a
// UTF-8 sequence): every rune that is not RE2's ASCII `\s` becomes one space, and so does every byte of an
// invalid sequence (Go's regexp reads each as a `RuneError` of width 1).
pub(crate) fn replace_non_whitespace(s: &[u8]) -> String {
    let mut out = String::with_capacity(s.len());
    for chunk in s.utf8_chunks() {
        for c in chunk.valid().chars() {
            if matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' ') {
                out.push(c);
            } else {
                out.push(' ');
            }
        }
        for _ in chunk.invalid() {
            out.push(' ');
        }
    }
    out
}

/// Rust-only: Go's `diag.Loc().Pos()` / `End()`: `-1` for a diagnostic without a location.
fn diag_pos(diag: P<Diagnostic>) -> i64 {
    diag.pos().map_or(-1, i64::from)
}

fn diag_end(diag: P<Diagnostic>) -> i64 {
    diag.end().map_or(-1, i64::from)
}

// go test: internal/effecttest/baseline.go:62 generateErrorBaseline 19301afa68f659e8
// generateErrorBaseline creates an error baseline string matching the TypeScript harness format.
pub(crate) fn generate_error_baseline(
    effect_version: &str,
    input_files: &[TestFile],
    diagnostics: &[P<Diagnostic>],
) -> String {
    let mut sb = String::new();

    sb.push_str("=== Metadata ===\n");
    let _ =
        writeln!(sb, "Effect version: {}", normalize_effect_version_for_baseline(effect_version));
    sb.push('\n');

    // Filter out diagnostics from /node_modules/ files — these are internal
    // dependency errors (e.g. incomplete type declarations in third-party
    // packages) that are not relevant to Effect diagnostic test baselines.
    let mut filtered: Vec<P<Diagnostic>> = Vec::new();
    for &diag in diagnostics {
        if diag.file().is_some_and(|f| f.file_name().contains("/node_modules/")) {
            continue;
        }
        filtered.push(diag);
    }

    // Sort diagnostics by file and position
    let mut sorted_diags = filtered.clone();
    ets_go::sort::slice(&mut sorted_diags, |a, b| {
        if a.file() != b.file() {
            let fi = a.file().map_or_else(String::new, |f| f.file_name().to_string());
            let fj = b.file().map_or_else(String::new, |f| f.file_name().to_string());
            return fi < fj;
        }
        diag_pos(*a) < diag_pos(*b)
    });

    // Write error summary at the top
    for &diag in &sorted_diags {
        let file_name =
            diag.file().map_or_else(|| "<global>".to_string(), |f| f.file_name().to_string());
        let mut line = 0usize;
        let mut char_ = 0usize;
        if let Some(file) = diag.file() {
            if diag_pos(diag) >= 0 {
                let (l, c) = ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(
                    &*file,
                    diag_pos(diag) as u32,
                );
                line = l;
                char_ = c as usize;
            }
        }
        let category_str = category_to_string(diag.category());
        let _ = writeln!(
            sb,
            "{}({},{}): {} TS{}: {}",
            file_name,
            line + 1,
            char_ + 1,
            category_str,
            diag.code(),
            diag.localize()
        );
    }

    sb.push('\n');

    // For each input file, show the file content with error markers
    for file in input_files {
        // Filter diagnostics for this file
        let mut file_errors: Vec<P<Diagnostic>> = Vec::new();
        for &diag in &sorted_diags {
            if diag.file().is_some_and(|f| f.file_name() == file.unit_name) {
                file_errors.push(diag);
            }
        }

        let _ = write!(sb, "\n==== {} ({} errors) ====\n", file.unit_name, file_errors.len());

        // Compute line starts for position calculations
        let line_starts = ets_shim::core::compute_ecma_line_starts(&file.content);
        let lines: Vec<&str> = LINE_DELIMITER.split(&file.content).collect();

        for (line_index, &line) in lines.iter().enumerate() {
            // Strip trailing \r if present
            let line = line.strip_suffix('\r').unwrap_or(line).as_bytes();

            let this_line_start = i64::from(line_starts[line_index]);
            let next_line_start = if line_index == lines.len() - 1 {
                file.content.len() as i64
            } else {
                i64::from(line_starts[line_index + 1])
            };

            // Emit this line from the original file with 4-space indentation
            sb.push_str("    ");
            sb.push_str(&String::from_utf8_lossy(line));
            sb.push('\n');

            // Check each error to see if it applies to this line
            for &err_diag in &file_errors {
                let err_start = diag_pos(err_diag);
                let mut err_len = diag_end(err_diag) - diag_pos(err_diag);
                if err_len <= 0 {
                    err_len = 1;
                }
                let end = err_start + err_len;

                // Does this error start or continue on this line?
                if end >= this_line_start
                    && (err_start < next_line_start || line_index == lines.len() - 1)
                {
                    // How many characters from the start of this line the error starts at
                    let relative_offset = err_start - this_line_start;
                    // How many characters of the error are on this line
                    let length = err_len - 0.max(this_line_start - err_start);
                    // Calculate the start of the squiggle
                    let squiggle_start = 0.max(relative_offset);

                    // Write squiggle line with indentation
                    sb.push_str("    ");
                    let line_len = line.len() as i64;
                    sb.push_str(&replace_non_whitespace(
                        &line[..squiggle_start.min(line_len) as usize],
                    ));
                    let squiggle_end = squiggle_start.max((squiggle_start + length).min(line_len));
                    if squiggle_end > squiggle_start {
                        sb.push_str(&"~".repeat(ets_go::utf8::rune_count(
                            &line[squiggle_start as usize..squiggle_end as usize],
                        )));
                    }
                    sb.push('\n');

                    // If the error ends on this line (or we're at EOF), emit the error message
                    if line_index == lines.len() - 1 || next_line_start > end {
                        let _ = writeln!(
                            sb,
                            "!!! {} TS{}: {}",
                            category_to_string(err_diag.category()),
                            err_diag.code(),
                            err_diag.localize()
                        );

                        append_related_diagnostics(&mut sb, err_diag.related_information(), 0);
                    }
                }
            }
        }
    }

    sb
}

// go test: internal/effecttest/baseline.go:192 normalizeEffectVersionForBaseline 3d34a0a95171215d
pub(crate) fn normalize_effect_version_for_baseline(version: &str) -> &str {
    match version.split_once('-') {
        Some((base, _)) => base,
        None => version,
    }
}

// go test: internal/effecttest/baseline.go:200 appendRelatedDiagnostics b34248e3d7c0813e
pub(crate) fn append_related_diagnostics(sb: &mut String, related: &[P<Diagnostic>], depth: usize) {
    let indent = "  ".repeat(depth);
    for &info in related {
        if let Some(file) = info.file() {
            let (rel_line, rel_char) =
                ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(
                    &*file,
                    diag_pos(info) as u32,
                );
            let _ = writeln!(
                sb,
                "{}!!! related TS{} {}({},{}): {}",
                indent,
                info.code(),
                file.file_name(),
                rel_line + 1,
                rel_char as usize + 1,
                info.localize()
            );
        }
        append_related_diagnostics(sb, info.related_information(), depth + 1);
    }
}

// go test: internal/effecttest/baseline.go:213 categoryToString 1e6e9fd4c7026e60
// categoryToString converts a diagnostic category to its lowercase string representation.
pub(crate) fn category_to_string(category: Category) -> &'static str {
    match category {
        Category::Error => "error",
        Category::Warning => "warning",
        Category::Suggestion => "suggestion",
        Category::Message => "message",
    }
}

// go test: internal/effecttest/baseline.go:229 DoPipingFlowBaseline 8d0db17ff883a5d8
// DoPipingFlowBaseline generates a .pipings.txt baseline for Effect tests.
//
// Rust: Go passes the checker alone (`c.Program()` is its program); the program is passed next to it.
pub fn do_piping_flow_baseline(
    t: &T,
    baseline_name: &str,
    program: &'static Program,
    c: &mut Checker,
    input_files: &[TestFile],
    source_file_getter: impl Fn(&str) -> Option<P<SourceFile>>,
    subfolder: &str,
) {
    let content = generate_piping_flow_baseline(program, c, input_files, source_file_getter);
    run_effect_baseline(t, &format!("{baseline_name}.pipings.txt"), &content, subfolder);
}

// go test: internal/effecttest/baseline.go:242 generatePipingFlowBaseline 13a1db136601c1ea
// generatePipingFlowBaseline creates a piping flow baseline string.
pub(crate) fn generate_piping_flow_baseline(
    program: &'static Program,
    c: &mut Checker,
    input_files: &[TestFile],
    source_file_getter: impl Fn(&str) -> Option<P<SourceFile>>,
) -> String {
    let mut sb = String::new();
    let mut tp = ets_typeparser::new_type_parser(program, c);

    for file in input_files {
        let Some(sf) = source_file_getter(&file.unit_name) else {
            let _ = writeln!(sb, "==== {} (0 flows) ====", file.unit_name);
            continue;
        };

        let flows = tp.piping_flows(sf, true);
        let flows = ets_readonly::new(flows.as_deref());
        let _ = writeln!(sb, "==== {} ({} flows) ====", file.unit_name, flows.len());

        for (_, flow) in flows.all() {
            sb.push_str("\n=== Piping Flow ===\n");

            // Location
            let (start_line, start_char) =
                ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(
                    &*sf,
                    flow.node().pos().expect_source("piping flow node"),
                );
            let (end_line, end_char) =
                ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(
                    &*sf,
                    flow.node().end().expect_source("piping flow node"),
                );
            let _ = writeln!(
                sb,
                "Location: {}:{} - {}:{}",
                start_line + 1,
                start_char as usize + 1,
                end_line + 1,
                end_char as usize + 1
            );

            // Node text
            let node_text =
                ets_shim::scanner::get_source_text_of_node_from_source_file(sf, flow.node(), false);
            let _ = writeln!(sb, "Node: {}", escape_newlines(&node_text));

            // Node kind
            let _ = writeln!(sb, "Node Kind: Kind{:?}", flow.node().kind());
            sb.push('\n');

            // Subject
            let subject = flow.subject();
            let subject_text = subject.node().map_or_else(String::new, |node| {
                ets_shim::scanner::get_source_text_of_node_from_source_file(sf, node, false)
            });
            let _ = writeln!(sb, "Subject: {}", escape_newlines(&subject_text));
            if let Some(out_type) = subject.out_type(&mut tp) {
                let type_text = tp.checker().type_to_string_ex(
                    out_type,
                    None,
                    TypeFormatFlags::NoTruncation,
                    None,
                );
                let _ = writeln!(sb, "Subject Type: {type_text}");
            } else {
                sb.push_str("Subject Type: \n");
            }
            sb.push('\n');

            // Transformations
            let transformations = flow.transformations();
            let _ = writeln!(sb, "Transformations ({}):", transformations.len());
            for (i, t) in transformations.all() {
                let callee_text = ets_shim::scanner::get_source_text_of_node_from_source_file(
                    sf,
                    t.callee(),
                    false,
                );

                let mut args_display = "(constant)".to_string();
                if t.args().len() > 0 {
                    let mut arg_texts: Vec<String> = Vec::new();
                    for (_, &arg) in t.args().all() {
                        arg_texts.push(escape_newlines(
                            &ets_shim::scanner::get_source_text_of_node_from_source_file(
                                sf, arg, false,
                            ),
                        ));
                    }
                    args_display = format!("[{}]", arg_texts.join(", "));
                }

                let mut out_type_str = String::new();
                if let Some(out_type) = t.out_type(&mut tp) {
                    out_type_str = tp.checker().type_to_string_ex(
                        out_type,
                        None,
                        TypeFormatFlags::NoTruncation,
                        None,
                    );
                }

                let _ = writeln!(sb, "  [{}] kind: {}", i, t.kind().as_str());
                let _ = writeln!(sb, "      callee: {}", escape_newlines(&callee_text));
                let _ = writeln!(sb, "      args: {args_display}");
                let _ = writeln!(sb, "      outType: {out_type_str}");
            }
        }
    }

    sb
}

// go test: internal/effecttest/baseline.go:320 escapeNewlines 581540f2cb48fd87
// escapeNewlines replaces newlines with \n for baseline display.
pub(crate) fn escape_newlines(s: &str) -> String {
    let s = s.replace("\r\n", "\\n");
    s.replace('\n', "\\n")
}

// go test: internal/effecttest/baseline.go:327 DoLayerGraphBaseline ad9a05973d3da964
// DoLayerGraphBaseline generates a .layers.txt baseline for Effect tests.
//
// Rust: Go passes the checker alone (`c.Program()` is its program); the program is passed next to it.
pub fn do_layer_graph_baseline(
    t: &T,
    baseline_name: &str,
    program: &'static Program,
    c: &mut Checker,
    input_files: &[TestFile],
    source_file_getter: impl Fn(&str) -> Option<P<SourceFile>>,
    subfolder: &str,
) {
    let content = generate_layer_graph_baseline(program, c, input_files, source_file_getter);
    run_effect_baseline(t, &format!("{baseline_name}.layers.txt"), &content, subfolder);
}

// go test: internal/effecttest/baseline.go:340 generateLayerGraphBaseline 1c704f9945be0c79
// generateLayerGraphBaseline creates a layer graph baseline string.
//
// Rust: Go's checker argument of the layergraph functions is the parser's checker (`tp.checker()`).
pub(crate) fn generate_layer_graph_baseline(
    program: &'static Program,
    c: &mut Checker,
    input_files: &[TestFile],
    source_file_getter: impl Fn(&str) -> Option<P<SourceFile>>,
) -> String {
    // Collect exported const declarations with Layer-typed initializers
    struct LayerExport {
        name: String,
        initializer: P<Node>,
    }

    let mut sb = String::new();
    let mut tp = ets_typeparser::new_type_parser(program, c);

    // Read follow depth from program-level plugin options (populated from tsconfig.json)
    let mut follow_depth = 0;
    if let Some(effect_config) = program.options().effect.get() {
        follow_depth = EffectPluginOptions::get_layer_graph_follow_depth(Some(&effect_config));
    }

    for file in input_files {
        let Some(sf) = source_file_getter(&file.unit_name) else {
            let _ = writeln!(sb, "==== {} (0 layer exports) ====", file.unit_name);
            continue;
        };

        let mut exports: Vec<LayerExport> = Vec::new();

        for &stmt in sf.statements().nodes() {
            if stmt.kind() != Kind::VariableStatement {
                continue;
            }
            // Check for export modifier
            if !ets_shim::ast::has_syntactic_modifier(stmt, ModifierFlags::Export) {
                continue;
            }
            let decl_list = stmt.as_variable_statement().declaration_list();
            for &decl in decl_list.as_variable_declaration_list().declarations().nodes() {
                if decl.kind() != Kind::VariableDeclaration {
                    continue;
                }
                let vd = decl.as_variable_declaration();
                let Some(initializer) = vd.initializer() else {
                    continue;
                };
                if vd.name().kind() != Kind::Identifier {
                    continue;
                }
                let t = tp.get_type_at_location(initializer);
                if !tp.is_layer_type(t) {
                    continue;
                }
                exports.push(LayerExport { name: vd.name().text().to_string(), initializer });
            }
        }

        let _ = writeln!(sb, "==== {} ({} layer exports) ====", file.unit_name, exports.len());

        for export in &exports {
            let _ = write!(sb, "\n=== {} ===\n", export.name);

            let opts = ExtractLayerGraphOptions {
                follow_symbols_depth: follow_depth,
                array_literal_as_merge: false,
                explode_only_layer_calls: false,
                ..Default::default()
            };

            let full_graph =
                ets_layergraph::extract_layer_graph(&mut tp, &[export.initializer], Some(sf), opts);
            let outline_graph = ets_layergraph::extract_outline_graph(&mut tp, &full_graph);
            let providers_and_requirers =
                ets_layergraph::extract_providers_and_requirers(tp.checker(), &full_graph);

            sb.push_str("\n--- output ---\n");
            sb.push_str(&ets_layergraph::format_layer_graph(tp.checker(), &full_graph, Some(sf)));
            sb.push('\n');

            sb.push_str("\n--- nested ---\n");
            sb.push_str(&ets_layergraph::format_nested_layer_graph(
                tp.checker(),
                &full_graph,
                Some(sf),
            ));
            sb.push('\n');

            sb.push_str("\n--- outline ---\n");
            sb.push_str(&ets_layergraph::format_outline_graph(
                tp.checker(),
                &outline_graph,
                Some(sf),
            ));
            sb.push('\n');

            sb.push_str("\n--- quickinfo ---\n");
            sb.push_str(&ets_layergraph::format_quick_info(
                tp.checker(),
                &providers_and_requirers,
                Some(sf),
            ));
            sb.push('\n');
        }
    }

    sb
}

// go test: internal/effecttest/baseline.go:442 runEffectBaseline c9b502bf33c93383
// runEffectBaseline compares actual output with reference baseline.
// It only writes to the filesystem when the actual content differs from the
// reference (or when the reference doesn't exist yet), so that Go's test
// cache remains valid for passing tests.
//
// Rust: the Go runner owns the reference, so a missing reference is a mismatch here (Go creates it) and only the
// local file is written. The outcome is recorded by `baseline_results::report`, which fails a mismatch as Go does.
pub(crate) fn run_effect_baseline(t: &T, file_name: &str, actual: &str, subfolder: &str) {
    let local_dir = baseline_local_path(subfolder);
    let reference_dir = baseline_reference_path(subfolder);

    let local_path = local_dir.join(file_name);
    let reference_path = reference_dir.join(file_name);
    let baseline = format!("{subfolder}/{file_name}");

    // Read reference first to decide whether any writes are needed.
    let expected = match std::fs::read(&reference_path) {
        Ok(reference_content) => Some(String::from_utf8_lossy(&reference_content).into_owned()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => t.fatal(format!("Failed to read reference baseline: {err}")),
    };

    // Compare actual against reference.
    if expected.as_deref() == Some(actual) {
        baseline_results::report(t, &baseline, baseline_results::Outcome::Match, String::new);
        return;
    }

    // Mismatch — write local baseline so developers can inspect the diff.
    if let Err(err) = std::fs::create_dir_all(&local_dir) {
        t.fatal(format!("Failed to create local baseline directory: {err}"));
    }
    if let Err(err) = std::fs::write(&local_path, actual) {
        t.fatal(format!("Failed to write local baseline: {err}"));
    }

    baseline_results::report(t, &baseline, baseline_results::Outcome::Mismatch, || {
        let diff = diff_text(
            &reference_path.to_string_lossy(),
            &local_path.to_string_lossy(),
            expected.as_deref().unwrap_or_default(),
            actual,
        );
        // Indent the diff for readability
        let diff_lines: Vec<String> = diff.split('\n').map(|line| format!("  {line}")).collect();
        format!("Baseline mismatch:\n{}", diff_lines.join("\n"))
    });
}
