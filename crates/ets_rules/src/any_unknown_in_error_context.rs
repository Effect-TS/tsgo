// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::checker::{Signature, Type, TypeFlags};
use ets_shim::diagnostics as tsdiag;

// go: internal/rules/any_unknown_in_error_context.go:18 AnyUnknownInErrorContext be5eb0d55ec9b5e3
// AnyUnknownInErrorContext detects 'any' or 'unknown' types in the error (E) or
// requirements (R) channels of Effect and Layer types.
pub static ANY_UNKNOWN_IN_ERROR_CONTEXT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "anyUnknownInErrorContext",
    group: "correctness",
    description: "Detects 'any' or 'unknown' types in Effect error or requirements channels",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::X_0_EFFECT_ANY_UNKNOWN_IN_ERROR_CONTEXT.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        struct MatchEntry {
            node: P<Node>,
            message_text: String,
            flagged: AnyUnknownChannels,
            start: i64,
            end: i64,
        }

        let mut matches: Vec<MatchEntry> = Vec::new();

        // Stack-based traversal
        let mut node_to_visit: Vec<P<Node>> = Vec::new();
        ctx.source_file.as_node().for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });

        // Pop from the end (stack)
        while let Some(node) = node_to_visit.pop() {
            // Skip type nodes
            if ast::is_type_node(node) {
                continue;
            }
            // Skip type alias declarations
            if node.kind() == Kind::TypeAliasDeclaration {
                continue;
            }
            // Skip interface declarations
            if node.kind() == Kind::InterfaceDeclaration {
                continue;
            }
            // Skip "as any" expressions
            if node.kind() == Kind::AsExpression {
                if let Some(type_node) = node.type_node() {
                    if type_node.kind() == Kind::AnyKeyword {
                        continue;
                    }
                }
            }

            // If this is a parameter, property, or variable declaration with explicit
            // Effect or Layer type annotation, skip it entirely (user intentionally typed it)
            if node.kind() == Kind::Parameter
                || node.kind() == Kind::PropertyDeclaration
                || node.kind() == Kind::VariableDeclaration
            {
                if let Some(type_node) = node.type_node() {
                    if ctx.type_parser.strict_effect_type_at_location(type_node).is_some() {
                        continue;
                    }
                    if ctx.type_parser.layer_type_at_location(type_node).is_some() {
                        continue;
                    }
                }
            }

            // Enqueue children for visiting
            node.for_each_child(&mut |child| {
                node_to_visit.push(child);
                false
            });

            if ets_typeparser::type_equivalent_node(node).1 {
                continue;
            }

            // For call expressions, use the resolved signature's return type.
            // Try strict Effect type first, then Layer type
            let mut sig: Option<P<Signature>> = None;
            if node.kind() == Kind::CallExpression {
                sig = Some(ctx.checker().get_resolved_signature_exported(node));
            }
            // Rust: Go's `eff`/`layer` assignments are one `(eff, layer)` binding.
            let (eff, layer) = if let Some(sig) = sig {
                let t = ctx.checker().get_return_type_of_signature_exported(sig);
                match ctx.type_parser.strict_effect_type(t) {
                    Some(eff) => (Some(eff), None),
                    None => (None, ctx.type_parser.layer_type(t)),
                }
            } else {
                match ctx.type_parser.strict_effect_type_at_location(node) {
                    Some(eff) => (Some(eff), None),
                    None => (None, ctx.type_parser.layer_type_at_location(node)),
                }
            };

            let mut e_type: Option<P<Type>> = None;
            let mut r_type: Option<P<Type>> = None;
            if let Some(eff) = eff {
                e_type = Some(eff.e());
                r_type = Some(eff.r());
            } else if let Some(layer) = layer {
                e_type = Some(layer.e());
                r_type = Some(layer.r_in());
            }

            let (Some(e_type), Some(r_type)) = (e_type, r_type) else {
                continue;
            };

            let flagged = AnyUnknownChannels {
                r: r_type.flags() & (TypeFlags::Any | TypeFlags::Unknown),
                e: e_type.flags() & (TypeFlags::Any | TypeFlags::Unknown),
            };
            let has_any_unknown_r = !flagged.r.is_empty();
            let has_any_unknown_e = !flagged.e.is_empty();

            if !has_any_unknown_r && !has_any_unknown_e {
                continue;
            }

            // Build the channel descriptions
            let mut channels: Vec<String> = Vec::new();
            if has_any_unknown_r {
                let mut type_name = "unknown";
                if r_type.flags().intersects(TypeFlags::Any) {
                    type_name = "any";
                }
                channels.push(format!("{type_name} in the requirements channel"));
            }
            if has_any_unknown_e {
                let mut type_name = "unknown";
                if e_type.flags().intersects(TypeFlags::Any) {
                    type_name = "any";
                }
                channels.push(format!("{type_name} in the error channel"));
            }

            // Compose the diagnostic message
            let mut suggestions: Vec<String> = Vec::new();
            suggestions
                .push(format!("This has {} which is not recommended.", channels.join(" and ")));
            if has_any_unknown_r {
                suggestions.push(
                    "Only service identifiers should appear in the requirements channel."
                        .to_string(),
                );
            }
            if has_any_unknown_e {
                suggestions.push("Having an unknown or any error type is not useful. Consider instead using specific error types baked by Data.TaggedError for example.".to_string());
            }
            let message_text = suggestions.join("\n");

            let node_start =
                i64::from(ets_shim::scanner::get_token_pos_of_node(node, ctx.source_file, false));
            let node_end = node.end().source_offset().map_or(-1, i64::from);

            // Innermost-node deduplication: remove parent nodes that contain this
            // node when this node already reports all of their channels
            for i in (0..matches.len()).rev() {
                if matches[i].start <= node_start
                    && matches[i].end >= node_end
                    && flagged.covers(matches[i].flagged)
                {
                    matches.remove(i);
                }
            }

            matches.push(MatchEntry {
                node,
                message_text,
                flagged,
                start: node_start,
                end: node_end,
            });
        }

        // Report all innermost matching nodes
        let mut diags = Vec::new();
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                ast::get_source_file_of_node(m.node).expect("a node of the source file"),
                ctx.get_error_range(m.node),
                &tsdiag::X_0_EFFECT_ANY_UNKNOWN_IN_ERROR_CONTEXT,
                &[],
                &[&m.message_text],
            ));
        }

        diags
    },
});

// go: internal/rules/any_unknown_in_error_context.go:189 anyUnknownChannels 012a7bb33777ba45
// anyUnknownChannels holds TypeFlagsAny or TypeFlagsUnknown for each channel
// that has one, and zero for a channel that has neither.
#[derive(Clone, Copy)]
pub(crate) struct AnyUnknownChannels {
    r: TypeFlags,
    e: TypeFlags,
}

impl AnyUnknownChannels {
    // go: internal/rules/any_unknown_in_error_context.go:196 anyUnknownChannels.covers c0ec76d0a0b919ef
    // covers reports whether c flags every channel that other flags, with the
    // same any or unknown.
    pub(crate) fn covers(self, other: AnyUnknownChannels) -> bool {
        (other.r.is_empty() || other.r == self.r) && (other.e.is_empty() || other.e == self.e)
    }
}
