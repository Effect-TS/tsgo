use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_TYPE_ASSERTION_UNSAFELY_NARROWS_THE_ERROR_OR_REQUIREMENTS_CHANNELS_EFFECT_UNSAFE_EFFECT_TYPE_ASSERTION;

// go: internal/rules/unsafe_effect_type_assertion.go:14 UnsafeEffectTypeAssertion 96d442f4eaaffa6b
pub static UNSAFE_EFFECT_TYPE_ASSERTION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unsafeEffectTypeAssertion",
    group: "correctness",
    description: "Detects unsafe type assertions that narrow Effect, Stream, or Layer error or requirements channels",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unsafe_effect_type_assertion(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let related = unsafe_effect_type_assertion_related_information(Some(ctx), m);
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &related, &[]));
        }
        diags
    },
});

// go: internal/rules/unsafe_effect_type_assertion.go:36 UnsafeEffectTypeAssertionChannel e220334dfe7ab62d
#[derive(Clone)]
pub struct UnsafeEffectTypeAssertionChannel {
    pub name: &'static str,
    pub original: String,
    pub asserted: String,
}

// go: internal/rules/unsafe_effect_type_assertion.go:42 UnsafeEffectTypeAssertionMatch d3232df42345c25f
#[derive(Clone)]
pub struct UnsafeEffectTypeAssertionMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub assertion_node: P<Node>,
    pub expression_node: P<Node>,
    pub location_node: Option<P<Node>>,
    pub channels: Vec<UnsafeEffectTypeAssertionChannel>,
}

// go: internal/rules/unsafe_effect_type_assertion.go:51 AnalyzeUnsafeEffectTypeAssertion 3114dd5858d77025
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); the parser and checker are never nil.
pub fn analyze_unsafe_effect_type_assertion(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<UnsafeEffectTypeAssertionMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    // Rust: Go's `parseEffectStreamOrLayer` closure is a nested function; `(e, r, ok)` is an `Option`.
    fn parse_effect_stream_or_layer(
        tp: &mut TypeParser<'_>,
        t: P<Type>,
    ) -> Option<(P<Type>, P<Type>)> {
        if let Some(effect) = tp.effect_type(t) {
            return Some((effect.e(), effect.r()));
        }
        if let Some(stream) = tp.stream_type(t) {
            return Some((stream.e(), stream.r()));
        }
        if let Some(layer) = tp.layer_type(t) {
            return Some((layer.e(), layer.r_in()));
        }
        None
    }
    let mut nodes_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        nodes_to_visit.push(child);
        false
    });

    while let Some(node) = nodes_to_visit.pop() {
        node.for_each_child(&mut |child| {
            nodes_to_visit.push(child);
            false
        });

        if node.kind() != Kind::AsExpression && node.kind() != Kind::TypeAssertionExpression {
            continue;
        }

        let Some(expr) = node.expression() else {
            continue;
        };
        if tp.node_cannot_be_effect_library_type(expr) {
            continue;
        }

        let original_type = tp.get_type_at_location(expr);
        let asserted_type = tp.get_type_at_location(node);
        let (Some(original_type), Some(asserted_type)) = (original_type, asserted_type) else {
            continue;
        };

        let Some((original_e, original_r)) = parse_effect_stream_or_layer(tp, original_type) else {
            continue;
        };

        let Some((asserted_e, asserted_r)) = parse_effect_stream_or_layer(tp, asserted_type) else {
            continue;
        };

        let mut channels = Vec::with_capacity(2);
        if !is_any_type(Some(original_e))
            && !tp.checker().is_type_assignable_to(original_e, asserted_e)
        {
            channels.push(UnsafeEffectTypeAssertionChannel {
                name: "error",
                original: tp.checker().type_to_string_exported(original_e),
                asserted: tp.checker().type_to_string_exported(asserted_e),
            });
        }
        if !is_any_type(Some(original_r))
            && !tp.checker().is_type_assignable_to(original_r, asserted_r)
        {
            channels.push(UnsafeEffectTypeAssertionChannel {
                name: "requirements",
                original: tp.checker().type_to_string_exported(original_r),
                asserted: tp.checker().type_to_string_exported(asserted_r),
            });
        }
        if channels.is_empty() {
            continue;
        }

        let mut location_node = node;
        if let Some(type_node) = node.type_node() {
            location_node = type_node;
        }

        matches.push(UnsafeEffectTypeAssertionMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, node),
            assertion_node: node,
            expression_node: expr,
            location_node: Some(location_node),
            channels,
        });
    }

    matches
}

// go: internal/rules/unsafe_effect_type_assertion.go:143 isAnyType c8a188e72b87a725
pub(crate) fn is_any_type(t: Option<P<Type>>) -> bool {
    t.is_some_and(|t| t.flags().intersects(TypeFlags::Any))
}

// go: internal/rules/unsafe_effect_type_assertion.go:147 unsafeEffectTypeAssertionRelatedInformation 8fa0f1b20fb7c26e
pub(crate) fn unsafe_effect_type_assertion_related_information(
    ctx: Option<&Context<'_, '_>>,
    m: &UnsafeEffectTypeAssertionMatch,
) -> Vec<P<Diagnostic>> {
    let Some(ctx) = ctx else {
        return Vec::new();
    };
    if m.channels.is_empty() {
        return Vec::new();
    }

    let location_node = m.location_node.unwrap_or(m.assertion_node);

    let mut related = Vec::with_capacity(m.channels.len());
    for channel in &m.channels {
        related.push(ctx.new_diagnostic(
            m.source_file,
            ets_shim::scanner::get_error_range_for_node(m.source_file, location_node),
            &tsdiag::THE_0_CHANNEL_IS_NARROWED_FROM_1_TO_2_EFFECT_UNSAFE_EFFECT_TYPE_ASSERTION,
            &[],
            &[channel.name, &channel.original, &channel.asserted],
        ));
    }
    related
}
