use ets_shim::ast::SourceFile;
use ets_shim::checker::Type;
use ets_shim::core::P;
use ets_shim::core::collections::OrderedMap;
use ets_shim::core::context::Context;
use ets_shim::ls::change;
use ets_shim::ls::lsconv;
use ets_shim::ls::lsutil;
use ets_shim::lsproto;

use crate::*;

impl TypeParser<'_> {
    // go: etsgoapi/structural_schema.go:20 TypeParser.RenderStructuralSchemaStatements d32e0f3326055b7d
    // RenderStructuralSchemaStatements converts a resolved TypeScript type into
    // rendered Effect Schema statements using the same structural generator as the
    // structuralTypeToSchema refactor.
    //
    // Rust: Go's nil result is `None`. The receiver, its inner parser, program and checker are never nil here, so
    // only the argument guards remain.
    pub fn render_structural_schema_statements(
        &mut self,
        source_file: Option<P<SourceFile>>,
        name: &str,
        t: Option<P<Type>>,
        is_exported: bool,
    ) -> Option<Vec<String>> {
        let (Some(source_file), Some(t)) = (source_file, t) else {
            return None;
        };
        if name.is_empty() {
            return None;
        }

        let file_name = source_file.file_name().to_string();
        let text = source_file.text();
        let converters =
            lsconv::new_converters(lsproto::PositionEncodingKind::UTF8, move |name: &str| {
                if name == file_name {
                    return Some(lsconv::compute_lsp_line_starts(text));
                }
                None
            });
        let raw_tracker = change::new_tracker(
            &Context::background(),
            &self.program.options(),
            lsutil::get_default_format_code_settings(),
            converters,
        );
        let tracker = ets_rewriter::new_tracker(raw_tracker);

        let version = self.inner.supported_effect_version();
        let mut generator =
            ets_schemagen::new_structural_schema_gen(&tracker, source_file, version);
        let mut type_map = OrderedMap::default();
        type_map.insert(name.to_string(), t);
        let statements =
            generator.process(&mut self.inner, &type_map, source_file.as_node(), is_exported);
        if statements.is_empty() {
            return None;
        }

        let mut rendered = Vec::with_capacity(statements.len());
        for statement in statements {
            // Go: `change.Tracker_getNonformattedText(rawTracker, statement, sourceFile)`.
            let (text, _) = tracker.tracker.get_nonformatted_text(statement, source_file);
            if !text.is_empty() {
                rendered.push(text);
            }
        }
        Some(rendered)
    }
}
