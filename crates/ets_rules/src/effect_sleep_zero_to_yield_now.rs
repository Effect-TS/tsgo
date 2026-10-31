use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, OuterExpressionKinds, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};
use regex::Regex;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::CONSIDER_EFFECT_YIELD_NOW_FOR_COOPERATIVE_YIELDING_INSTEAD_OF_SLEEPING_FOR_ZERO_DURATION_EFFECT_EFFECT_SLEEP_ZERO_TO_YIELD_NOW;

// go: internal/rules/effect_sleep_zero_to_yield_now.go:19 EffectSleepZeroToYieldNow fce8fb32d5c3932d
pub static EFFECT_SLEEP_ZERO_TO_YIELD_NOW: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectSleepZeroToYieldNow",
    group: "style",
    description: "Suggests Effect.yieldNow for cooperative yielding instead of sleeping for zero duration",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_sleep_zero_to_yield_now(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_sleep_zero_to_yield_now.go:39 YieldNowForm b8963802ff0e8ed8
// go: internal/rules/effect_sleep_zero_to_yield_now.go:41 YieldNowCall 542c6f04ce224159
// Rust: Go's `iota + 1` constants; the zero value is never used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum YieldNowForm {
    Call = 1,
    Value = 2,
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:46 EffectSleepZeroToYieldNowMatch 8d12d5e28566ad43
#[derive(Clone)]
pub struct EffectSleepZeroToYieldNowMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub call: P<Node>,
    pub effect_module: Option<P<Node>>,
    pub form: YieldNowForm,
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:54 AnalyzeEffectSleepZeroToYieldNow f2022af02d6d859c
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_sleep_zero_to_yield_now(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectSleepZeroToYieldNowMatch> {
    let version = tp.detect_effect_version();
    let form = match version {
        EffectMajorVersion::V3 => YieldNowForm::Call,
        EffectMajorVersion::V4 => YieldNowForm::Value,
        EffectMajorVersion::Unknown => return Vec::new(),
    };

    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        version: EffectMajorVersion,
        form: YieldNowForm,
        node: P<Node>,
        matches: &mut Vec<EffectSleepZeroToYieldNowMatch>,
    ) -> bool {
        if node.kind() == Kind::CallExpression && is_ordinary_single_argument_call(node) {
            let call = node.as_call_expression();
            let callee = unwrap_zero_duration_expression(call.expression());
            if !callee.flags().intersects(NodeFlags::OptionalChain)
                && tp.is_node_reference_to_effect_module_api(callee, "sleep")
                && is_removable_zero_duration(tp, call.arguments().nodes()[0], version)
            {
                let mut receiver = None;
                if callee.kind() == Kind::PropertyAccessExpression {
                    let candidate = callee.as_property_access_expression().expression();
                    if is_imported_module_receiver(tp, candidate)
                        && tp
                            .is_expression_effect_module(unwrap_zero_duration_expression(candidate))
                    {
                        receiver = Some(candidate);
                    }
                }
                matches.push(EffectSleepZeroToYieldNowMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, call.expression()),
                    call: node,
                    effect_module: receiver,
                    form,
                });
            }
        }
        node.for_each_child(&mut |child| walk(tp, sf, version, form, child, matches));
        false
    }
    walk(tp, sf, version, form, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:99 unwrapZeroDurationExpression 09e3400c206d959b
pub(crate) fn unwrap_zero_duration_expression(node: P<Node>) -> P<Node> {
    ast::skip_outer_expressions(
        node,
        OuterExpressionKinds::Parentheses
            | OuterExpressionKinds::TypeAssertions
            | OuterExpressionKinds::Satisfies,
    )
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:103 isOrdinarySingleArgumentCall 449b85a4aa7eed12
pub(crate) fn is_ordinary_single_argument_call(node: P<Node>) -> bool {
    let call = node.as_call_expression();
    let arguments = call.arguments().nodes();
    !node.flags().intersects(NodeFlags::OptionalChain)
        && call.type_arguments().is_none()
        && arguments.len() == 1
        && arguments[0].kind() != Kind::SpreadElement
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:110 isImportedModuleReceiver 5b88b506b3a5a073
pub(crate) fn is_imported_module_receiver(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let node = unwrap_zero_duration_expression(node);
    if node.kind() == Kind::PropertyAccessExpression
        && !node.flags().intersects(NodeFlags::OptionalChain)
    {
        let root =
            unwrap_zero_duration_expression(node.as_property_access_expression().expression());
        if root.kind() != Kind::Identifier {
            return false;
        }
        let symbol = tp.get_symbol_at_location(root);
        return symbol.is_some_and(|symbol| {
            let declarations = symbol.declarations();
            declarations.len() == 1 && declarations[0].kind() == Kind::NamespaceImport
        });
    }
    if node.kind() != Kind::Identifier {
        return false;
    }
    let Some(symbol) = tp.get_symbol_at_location(node) else {
        return false;
    };
    let declarations = symbol.declarations();
    if declarations.len() != 1 {
        return false;
    }
    let kind = declarations[0].kind();
    kind == Kind::ImportSpecifier || kind == Kind::NamespaceImport
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:131 zeroDurationString 7c51699657b1e4b8
pub(crate) static ZERO_DURATION_STRING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(-?0+(?:\.0+)?)[ \t]+(nanos?|micros?|millis?|seconds?|minutes?|hours?|days?|weeks?)$",
    )
    .expect("valid regexp")
});

// go: internal/rules/effect_sleep_zero_to_yield_now.go:133 isRemovableZeroDuration 94811fcfc9945915
pub(crate) fn is_removable_zero_duration(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    version: EffectMajorVersion,
) -> bool {
    let node = unwrap_zero_duration_expression(node);
    if is_zero_duration_number(node, false) || is_zero_duration_number(node, true) {
        return true;
    }
    match node.kind() {
        Kind::StringLiteral | Kind::NoSubstitutionTemplateLiteral => {
            let Some(parts) = ZERO_DURATION_STRING.captures(node.text()) else {
                return false;
            };
            let (number, unit) = (&parts[1], &parts[2]);
            version != EffectMajorVersion::V3
                || !number.contains('.')
                || (!unit.starts_with("nano") && !unit.starts_with("micro"))
        }
        Kind::ArrayLiteralExpression => {
            let elements = node.as_array_literal_expression().elements().nodes();
            elements.len() == 2
                && is_zero_duration_number(elements[0], false)
                && is_zero_duration_number(elements[1], false)
        }
        Kind::PropertyAccessExpression => {
            let receiver = node.as_property_access_expression().expression();
            !node.flags().intersects(NodeFlags::OptionalChain)
                && is_imported_module_receiver(tp, receiver)
                && tp
                    .is_expression_effect_duration_module(unwrap_zero_duration_expression(receiver))
                && tp.is_node_reference_to_effect_duration_module_api(node, "zero")
        }
        Kind::CallExpression => {
            if !is_ordinary_single_argument_call(node) {
                return false;
            }
            let call = node.as_call_expression();
            let callee = unwrap_zero_duration_expression(call.expression());
            if callee.kind() != Kind::PropertyAccessExpression
                || callee.flags().intersects(NodeFlags::OptionalChain)
            {
                return false;
            }
            let receiver = callee.as_property_access_expression().expression();
            if !is_imported_module_receiver(tp, receiver)
                || !tp
                    .is_expression_effect_duration_module(unwrap_zero_duration_expression(receiver))
            {
                return false;
            }
            let unit = callee.as_property_access_expression().name().text();
            match unit {
                "nanos" | "micros" => {
                    is_zero_duration_number(call.arguments().nodes()[0], true)
                        && tp.is_node_reference_to_effect_duration_module_api(callee, unit)
                }
                "millis" | "seconds" | "minutes" | "hours" | "days" | "weeks" => {
                    is_zero_duration_number(call.arguments().nodes()[0], false)
                        && tp.is_node_reference_to_effect_duration_module_api(callee, unit)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

// go: internal/rules/effect_sleep_zero_to_yield_now.go:179 isZeroDurationNumber 4f0885e90f8a81d6
pub(crate) fn is_zero_duration_number(node: P<Node>, bigint: bool) -> bool {
    let mut node = unwrap_zero_duration_expression(node);
    if node.kind() == Kind::PrefixUnaryExpression {
        let unary = node.as_prefix_unary_expression();
        if unary.operator() != Kind::MinusToken && (bigint || unary.operator() != Kind::PlusToken) {
            return false;
        }
        node = unwrap_zero_duration_expression(unary.operand());
    }
    if bigint {
        if node.kind() != Kind::BigIntLiteral {
            return false;
        }
        let text = node.text().replace('_', "");
        let text = text.strip_suffix('n').unwrap_or(&text);
        return go_big_int_base0_is_zero(text) == Some(true);
    }
    if node.kind() != Kind::NumericLiteral {
        return false;
    }
    let number = ets_go::strconv::parse_float(node.text(), 64);
    matches!(number, Ok(n) if n == 0.0)
}

/// Rust-only: Go's `new(big.Int).SetString(s, 0)` reduced to what `isZeroDurationNumber` reads: `None` when Go's
/// `ok` is false, else whether the value is zero. Base 0 takes an optional sign, then a `0x`/`0X`, `0b`/`0B`,
/// `0o`/`0O` or `0` (octal) prefix, and every remaining character must be a digit of that base (the caller has
/// removed the underscores).
fn go_big_int_base0_is_zero(s: &str) -> Option<bool> {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let bytes = s.as_bytes();
    let (base, digits, prefix_digit) = if bytes.len() >= 2 && bytes[0] == b'0' {
        match bytes[1] {
            b'x' | b'X' => (16, &s[2..], false),
            b'b' | b'B' => (2, &s[2..], false),
            b'o' | b'O' => (8, &s[2..], false),
            _ => (8, &s[1..], true),
        }
    } else {
        (10, s, false)
    };
    // The octal prefix `0` is itself a digit; the other prefixes need at least one digit after them.
    if digits.is_empty() && !prefix_digit {
        return None;
    }
    let mut zero = true;
    for c in digits.chars() {
        let d = c.to_digit(36)?;
        if d >= base {
            return None;
        }
        if d != 0 {
            zero = false;
        }
    }
    Some(zero)
}
