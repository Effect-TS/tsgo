use std::fmt::Write;

use ets_go::strconv::quote;
use ets_go::testing::T;
use ets_shim::lsproto;

use crate::*;

// go test: internal/effecttest/document_symbols_baseline.go:12 DoDocumentSymbolsBaseline
// DoDocumentSymbolsBaseline generates a .symbols.txt baseline for Effect tests.
pub fn do_document_symbols_baseline(
    t: &T,
    baseline_name: &str,
    subfolder: &str,
    file_symbols: &[DocumentSymbolsFileResult],
) {
    let content = generate_document_symbols_baseline(file_symbols);
    run_effect_baseline(t, &(baseline_name.to_string() + ".symbols.txt"), &content, subfolder);
}

// go test: internal/effecttest/document_symbols_baseline.go:23 DocumentSymbolsFileResult
// DocumentSymbolsFileResult stores document-symbol results for one file.
#[derive(Clone, Debug)]
pub struct DocumentSymbolsFileResult {
    pub file_name: String,
    pub document_trees: Vec<Vec<lsproto::DocumentSymbol>>,
    pub flat_lists: Vec<Vec<lsproto::SymbolInformation>>,
}

// go test: internal/effecttest/document_symbols_baseline.go:29 generateDocumentSymbolsBaseline
pub(crate) fn generate_document_symbols_baseline(
    file_symbols: &[DocumentSymbolsFileResult],
) -> String {
    let mut sb = String::new();

    for (i, file_result) in file_symbols.iter().enumerate() {
        let _ = writeln!(sb, "==== {} ====", file_result.file_name);

        for (i, tree) in file_result.document_trees.iter().enumerate() {
            let _ = write!(sb, "\n-- hierarchical[{i}] --\n");
            if tree.is_empty() {
                sb.push_str("(none)\n");
            } else {
                write_document_symbol_tree(&mut sb, tree, 0);
            }
        }

        for (i, flat) in file_result.flat_lists.iter().enumerate() {
            let _ = write!(sb, "\n-- flat[{i}] --\n");
            if flat.is_empty() {
                sb.push_str("(none)\n");
                continue;
            }
            for symbol in flat {
                let container = symbol.container_name.clone().unwrap_or_default();
                let _ = writeln!(
                    sb,
                    "({}) {} container={} range={}",
                    symbol.kind.string(),
                    symbol.name,
                    quote(&container),
                    format_range(symbol.location.range),
                );
            }
        }

        if i + 1 < file_symbols.len() {
            sb.push('\n');
        }
    }

    sb
}

// go test: internal/effecttest/document_symbols_baseline.go:72 writeDocumentSymbolTree
pub(crate) fn write_document_symbol_tree(
    sb: &mut String,
    symbols: &[lsproto::DocumentSymbol],
    indent: usize,
) {
    for symbol in symbols {
        let mut detail_suffix = String::new();
        if let Some(detail) = &symbol.detail {
            detail_suffix = format!(" detail={}", quote(detail));
        }
        let _ = writeln!(
            sb,
            "{}({}) {}{} range={} selection={}",
            "  ".repeat(indent),
            symbol.kind.string(),
            symbol.name,
            detail_suffix,
            format_range(symbol.range),
            format_range(symbol.selection_range),
        );
        if let Some(children) = &symbol.children {
            write_document_symbol_tree(sb, children, indent + 1);
        }
    }
}

// go test: internal/effecttest/document_symbols_baseline.go:92 formatRange
pub(crate) fn format_range(r: lsproto::Range) -> String {
    format!(
        "{}:{}-{}:{}",
        r.start.line.wrapping_add(1),
        r.start.character.wrapping_add(1),
        r.end.line.wrapping_add(1),
        r.end.character.wrapping_add(1),
    )
}
