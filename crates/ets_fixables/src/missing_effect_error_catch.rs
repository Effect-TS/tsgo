use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_go::strconv;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node};
use ets_shim::checker::{LiteralValue, Type, TypeFlags};
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::CodeAction;
use ets_shim::scanner;
use ets_typeparser::{EffectMajorVersion, TypeParser};
use rustc_hash::FxHashSet;

// go: internal/fixables/missing_effect_error_catch.go:19 MissingEffectErrorCatchFix
pub static MISSING_EFFECT_ERROR_CATCH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "missingEffectErrorCatch",
    description: "Catch all errors with Effect.catch/catchAll",
    error_codes: vec![
        tsdiag::Missing_errors_0_in_the_expected_Effect_type_effect_missingEffectError.code(),
    ],
    fix_ids: vec![
        "missingEffectError_catch",
        "missingEffectError_catchAll",
        "missingEffectError_tagged",
    ],
    run: run_missing_effect_error_catch_fix,
});

// go: internal/fixables/missing_effect_error_catch.go:27 runMissingEffectErrorCatchFix
fn run_missing_effect_error_catch_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    struct FixCandidate {
        start: u32,
        end: u32,
        tags: Vec<String>,
    }

    let mut catch_candidate: Option<FixCandidate> = None;
    let mut tagged_candidate: Option<FixCandidate> = None;

    let matches = ets_rules::analyze_missing_effect_error(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        let Some(error_expr) = expression_node_for_missing_effect_error(Some(m.error_node)) else {
            continue;
        };

        let node_start_pos = scanner::get_token_pos_of_node(error_expr, sf, false);
        let node_end_pos = error_expr.end().expect_source("a parsed expression");

        // Offer catch/catchAll only when expected error type is `never`.
        if m.expected_error_type.flags().intersects(TypeFlags::Never)
            && catch_candidate.as_ref().is_none_or(|c| {
                node_start_pos < c.start || (node_start_pos == c.start && node_end_pos < c.end)
            })
        {
            catch_candidate =
                Some(FixCandidate { start: node_start_pos, end: node_end_pos, tags: Vec::new() });
        }

        // Offer catchTags only when all missing error members have a literal _tag.
        if let Some(tags) =
            collect_literal_missing_error_tags(&mut ctx.type_parser, &m.unhandled_errors)
            && tagged_candidate.as_ref().is_none_or(|c| {
                node_start_pos < c.start || (node_start_pos == c.start && node_end_pos < c.end)
            })
        {
            tagged_candidate =
                Some(FixCandidate { start: node_start_pos, end: node_end_pos, tags });
        }
    }

    let mut actions = Vec::new();

    if let Some(catch_candidate) = catch_candidate {
        let mut method_name = "catch";
        if ctx.type_parser.supported_effect_version() == EffectMajorVersion::V3 {
            method_name = "catchAll";
        }

        let description = format!("Catch all errors with Effect.{method_name}");
        let todo_message = format!("TODO: {method_name} not implemented");

        let start = ctx.byte_pos_to_lsp_position(catch_candidate.start);
        let end = ctx.byte_pos_to_lsp_position(catch_candidate.end);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                tracker.insert_text(sf, start, &format!("Effect.{method_name}("));
                tracker.insert_text(
                    sf,
                    end,
                    &format!(", () => Effect.dieMessage({}))", strconv::quote(&todo_message)),
                );
            }),
        }) {
            actions.push(action);
        }
    }

    if let Some(tagged_candidate) = tagged_candidate {
        let start = ctx.byte_pos_to_lsp_position(tagged_candidate.start);
        let end = ctx.byte_pos_to_lsp_position(tagged_candidate.end);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Catch unexpected errors with Effect.catchTag".to_string(),
            run: Box::new(move |tracker| {
                tracker.insert_text(sf, start, "Effect.catchTags(");
                tracker.insert_text(
                    sf,
                    end,
                    &format!(", {})", build_catch_tags_handlers_object(&tagged_candidate.tags)),
                );
            }),
        }) {
            actions.push(action);
        }
    }

    actions
}

// go: internal/fixables/missing_effect_error_catch.go:114 collectLiteralMissingErrorTags
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
fn collect_literal_missing_error_tags(
    tp: &mut TypeParser<'_>,
    missing: &[P<Type>],
) -> Option<Vec<String>> {
    if missing.is_empty() {
        return None;
    }

    let mut seen: FxHashSet<String> = FxHashSet::default();
    let mut tags = Vec::with_capacity(missing.len());
    for &missing_type in missing {
        let mut tag_type = tp.checker().get_type_of_property_of_type_exported(missing_type, "_tag");
        if tag_type.is_none() {
            tag_type = tp.get_type_of_property_by_name(missing_type, "_tag");
        }
        let tag_type = tag_type?;
        if !tag_type.flags().intersects(TypeFlags::StringLiteral) {
            return None;
        }
        let Some(LiteralValue::String(tag_value)) = tag_type.as_literal_type().value() else {
            return None;
        };
        if seen.insert(tag_value.to_string()) {
            tags.push(tag_value.to_string());
        }
    }

    tags.sort();
    Some(tags)
}

// go: internal/fixables/missing_effect_error_catch.go:146 buildCatchTagsHandlersObject
fn build_catch_tags_handlers_object(tags: &[String]) -> String {
    let mut b = String::new();
    b.push_str("{ ");
    for (i, tag) in tags.iter().enumerate() {
        if i > 0 {
            b.push_str(", ");
        }
        b.push_str(&strconv::quote(tag));
        b.push_str(": () => Effect.dieMessage(");
        b.push_str(&strconv::quote(&format!("TODO: catchTags() not implemented for {tag}")));
        b.push(')');
    }
    b.push_str(" }");
    b
}

// go: internal/fixables/missing_effect_error_catch.go:162 expressionNodeForMissingEffectError
fn expression_node_for_missing_effect_error(node: Option<P<Node>>) -> Option<P<Node>> {
    let node = node?;
    // Handle SatisfiesExpression before the general IsExpression check:
    // we want to wrap just the LHS expression, not the entire `expr satisfies Type`.
    if node.kind() == Kind::SatisfiesExpression {
        return Some(node.as_satisfies_expression().expression());
    }
    if ast::is_expression(node) {
        return Some(node);
    }
    match node.kind() {
        Kind::VariableDeclaration => {
            if let Some(initializer) = node.as_variable_declaration().initializer()
                && ast::is_expression(initializer)
            {
                return Some(initializer);
            }
        }
        Kind::ReturnStatement => {
            if let Some(expression) = node.as_return_statement().expression()
                && ast::is_expression(expression)
            {
                return Some(expression);
            }
        }
        Kind::ArrowFunction => {
            if let Some(body) = node.as_arrow_function().body()
                && ast::is_expression(body)
            {
                return Some(body);
            }
        }
        _ => {}
    }
    None
}
