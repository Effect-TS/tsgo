use std::sync::LazyLock;

use ets_etscore::{BRANDED_SCHEMA_CLASS_OPAQUE, ResolvedEffectPluginOptions, Severity};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::ADD_READONLY_BRAND_COLON_UNIQUE_SYMBOL_AS_THE_SECOND_TYPE_ARGUMENT_WITHOUT_A_BRAND_TYPE_SCRIPT_MAY_ACCEPT_STRUCTURALLY_COMPATIBLE_VALUES_WHERE_THIS_CLASS_INSTANCE_TYPE_IS_EXPECTED_EFFECT_SCHEMA_CLASS_MISSING_BRAND;

// go: internal/rules/schema_class_missing_brand.go:11 SchemaClassMissingBrand 071aec44232c1ada
pub static SCHEMA_CLASS_MISSING_BRAND: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaClassMissingBrand",
    group: "correctness",
    description: "Warns when Schema classes, errors, or Opaque classes omit the brand type argument",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        if ctx.type_parser.supported_effect_version() != EffectMajorVersion::V4 {
            return Vec::new();
        }
        if ResolvedEffectPluginOptions::get_branded_schema_classes(ctx.options).is_empty() {
            return Vec::new();
        }
        let mut diagnostics = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            node: P<Node>,
            diagnostics: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if (node.kind() == Kind::ClassDeclaration || node.kind() == Kind::ClassExpression)
                && let Some(self_) =
                    schema_class_missing_brand_self(ctx.type_parser, ctx.options, node)
            {
                diagnostics.push(ctx.new_diagnostic(
                    ctx.source_file,
                    ctx.get_error_range(self_),
                    MESSAGE,
                    &[],
                    &[],
                ));
            }
            node.for_each_child(&mut |child| walk(ctx, child, diagnostics));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, &mut diagnostics);
        diagnostics
    },
});

// go: internal/rules/schema_class_missing_brand.go:46 schemaClassMissingBrandSelf edf5dda71032b452
pub(crate) fn schema_class_missing_brand_self(
    tp: &mut TypeParser<'_>,
    options: Option<&ResolvedEffectPluginOptions>,
    class_node: P<Node>,
) -> Option<P<Node>> {
    let (mut self_, mut brand, mut kind) = schema_class_instance_self_and_brand(tp, class_node);
    if self_.is_none()
        && let Some(result) = tp.extends_schema_opaque(class_node)
    {
        (self_, brand, kind) =
            (Some(result.self_type_node()), result.brand_type_node(), BRANDED_SCHEMA_CLASS_OPAQUE);
    }
    if brand.is_some()
        || !ResolvedEffectPluginOptions::branded_schema_classes_includes(options, kind)
    {
        return None;
    }
    self_
}
