use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::NodeOptions;
use ets_rules::YieldInCallArgumentMatch;
use ets_shim::ast::{self, Kind, Node, NodeFlags, OuterExpressionKinds, SymbolFlags};
use ets_shim::checker::Checker;
use ets_shim::core::{P, TextRange, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;
use rustc_hash::FxHashMap;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_YIELD_ASTERISK_IS_USED_AS_A_CALL_ARGUMENT_YIELD_IT_INTO_A_CONST_FIRST_OR_MAP_THE_EFFECT_SO_RUNNING_THE_EFFECT_AND_BUILDING_THE_CALL_STAY_SEPARATE_STEPS_EFFECT_YIELD_IN_CALL_ARGUMENT;

// go: internal/fixables/yield_in_call_argument.go:19 YieldInCallArgumentFix 3a26dafd20b6ef4e
pub static YIELD_IN_CALL_ARGUMENT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "yieldInCallArgument",
    description: "Hoist yield* into a const",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["yieldInCallArgument_hoist"],
    run: run_yield_in_call_argument_fix,
});

// go: internal/fixables/yield_in_call_argument.go:29 runYieldInCallArgumentFix e9fceefb9b802d33
fn run_yield_in_call_argument_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_yield_in_call_argument(&mut ctx.type_parser, sf);
    let Some(m) = yield_in_call_argument_match_at(&matches, ctx.span).copied() else {
        return Vec::new();
    };
    let Some(statement) = m.statement else {
        return Vec::new();
    };

    let operand = m.yield_.expression().expect("a yield* match has an operand");
    let name = yield_in_call_argument_name(ctx.checker(), operand, statement);
    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Hoist yield* into a const".to_string(),
        run: Box::new(move |tracker| {
            let declaration = tracker.new_variable_declaration(
                tracker.new_identifier(alloc_str(&name)),
                None,
                None,
                Some(tracker.new_yield_expression(
                    Some(tracker.new_token(Kind::AsteriskToken)),
                    tracker.deep_clone_node(Some(operand)),
                )),
            );
            let statement_node = tracker.new_variable_statement(
                None,
                tracker.new_variable_declaration_list(
                    tracker.new_node_list(vec![declaration]),
                    NodeFlags::Const,
                ),
            );
            ast::set_parent_in_children(statement_node);

            // No flow edit inserts a statement or replaces a single argument, and
            // new or multi-argument calls have no flow, so the fix edits nodes directly.

            let pos = scanner::get_token_pos_of_node(statement, sf, true);
            // Rust: tsrs's `NodeOptions` has crate-private fields, so the literal is a default with Go's field set.
            let mut options = NodeOptions::default();
            options.suffix = type_union_to_schema_union_insertion_suffix(sf, pos);
            tracker.insert_node_at(sf, pos, statement_node, options);
            tracker.replace_node(
                sf,
                Some(m.argument),
                Some(tracker.new_identifier(alloc_str(&name))),
                None,
            );
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}

// go: internal/fixables/yield_in_call_argument.go:68 yieldInCallArgumentMatchAt 9aaf7408d4cbe738
// yieldInCallArgumentMatchAt picks the match whose location is exactly span,
// falling back to the first overlapping one. The exact match comes first because
// a nested yield* such as f(yield* g(yield* x)) lies inside the outer range.
fn yield_in_call_argument_match_at(
    matches: &[YieldInCallArgumentMatch],
    span: TextRange,
) -> Option<&YieldInCallArgumentMatch> {
    for m in matches {
        if m.location == span {
            return Some(m);
        }
    }
    matches.iter().find(|m| m.location.intersects(span) || span.contained_by(m.location))
}

// go: internal/fixables/yield_in_call_argument.go:84 yieldInCallArgumentVerbs 0d344c5ad43828a4
// yieldInCallArgumentVerbs maps a leading verb of the yielded name to its past
// participle, so `load` becomes `loaded` and `fetchUser` becomes `fetchedUser`.
static YIELD_IN_CALL_ARGUMENT_VERBS: LazyLock<FxHashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        FxHashMap::from_iter([("load", "loaded"), ("fetch", "fetched"), ("build", "built")])
    });

// go: internal/fixables/yield_in_call_argument.go:94 yieldInCallArgumentName ff0a04534d5b32bb
// yieldInCallArgumentName picks the hoisted const name from the yielded
// expression (its identifier or last property name, with a leading verb turned
// into a participle), falling back to `value`, then appends 2, 3, ... until no
// value with that name is visible at the statement.
fn yield_in_call_argument_name(c: &mut Checker, operand: P<Node>, statement: P<Node>) -> String {
    let mut base = "value".to_string();
    let operand = ast::skip_outer_expressions(
        operand,
        OuterExpressionKinds::Parentheses | OuterExpressionKinds::Assertions,
    );
    let mut segment = String::new();
    match operand.kind() {
        Kind::Identifier => segment = operand.text().to_string(),
        Kind::PropertyAccessExpression => {
            let name = operand.as_property_access_expression().name();
            if name.kind() == Kind::Identifier {
                segment = name.text().to_string();
            }
        }
        _ => {}
    }
    if !segment.is_empty() {
        let verb_end = segment
            .char_indices()
            .find(|&(_, r)| !go_unicode_is_lower(r))
            .map_or(segment.len(), |(index, _)| index);
        if let Some(participle) = YIELD_IN_CALL_ARGUMENT_VERBS.get(&segment[..verb_end]) {
            segment = (*participle).to_string() + &segment[verb_end..];
        }
        let token = scanner::string_to_token(&segment);
        if scanner::is_valid_identifier(&segment)
            && (token < Kind::FIRST_RESERVED_WORD || token > Kind::LAST_FUTURE_RESERVED_WORD)
        {
            base = segment;
        }
    }

    let mut is_free = |candidate: &str| -> bool {
        c.resolve_name_exported(
            candidate,
            Some(statement),
            SymbolFlags::Value | SymbolFlags::Alias,
            false,
        )
        .is_none()
    };
    if is_free(&base) {
        return base;
    }
    let mut i: i64 = 2;
    loop {
        let candidate = base.clone() + &i.to_string();
        if is_free(&candidate) {
            return candidate;
        }
        i += 1;
    }
}

/// Rust-only: Go's `unicode.IsLower`, as tsrs's `autoimport` port spells it. Go's table is the `Ll` category;
/// Rust's `char::is_lowercase` is the `Lowercase` property (`Ll` plus `Other_Lowercase`), which differs only for a few
/// modifier letters and symbols. ASCII, which the verb table is written in, is exact.
fn go_unicode_is_lower(r: char) -> bool {
    if r.is_ascii() {
        return r.is_ascii_lowercase();
    }
    r.is_lowercase()
}
