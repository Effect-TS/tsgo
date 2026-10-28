use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;
use rustc_hash::FxHashSet;

// go: internal/rules/prefer_effect_array.go:11 PreferEffectArray 6f24c3491b302ace
pub static PREFER_EFFECT_ARRAY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferEffectArray",
    group: "effectNative",
    description: "Suggests Effect Array APIs for native array member references, with semantic caveats",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::CONSIDER_EFFECT_SLASH_ARRAY_0_INSTEAD_OF_NATIVE_ARRAY_1_2_EFFECT_PREFER_EFFECT_ARRAY
            .code(),
    ],
    replaced_by: vec![],
    run: run_prefer_effect_array,
});

// go: internal/rules/prefer_effect_array.go:21 arrayMemberKind 56f92663d689a42a
// go: internal/rules/prefer_effect_array.go:23 arrayInstanceMember 8ee8ca8aa5a1f499
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ArrayMemberKind {
    InstanceMember,
    ConstructorMember,
}

// go: internal/rules/prefer_effect_array.go:28 arrayMethodPreference 6614c1057eed19b7
pub(crate) struct ArrayMethodPreference {
    pub(crate) kind: ArrayMemberKind,
    pub(crate) native_name: &'static str,
    pub(crate) effect_api: &'static str,
    pub(crate) caveat: &'static str,
}

/// Rust-only: Go's positional struct literal `{kind, nativeName, effectAPI, caveat}`.
const fn array_method_preference(
    kind: ArrayMemberKind,
    native_name: &'static str,
    effect_api: &'static str,
    caveat: &'static str,
) -> ArrayMethodPreference {
    ArrayMethodPreference { kind, native_name, effect_api, caveat }
}

// go: internal/rules/prefer_effect_array.go:35 arrayMethodPreferences 7f0ee91321eb15f5
pub(crate) static ARRAY_METHOD_PREFERENCES: &[ArrayMethodPreference] = &[
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "map",
        "map",
        "The callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "filter",
        "filter",
        "The callback type accepts (value, index), with no array parameter or thisArg; sparse-array holes are visited.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "every",
        "every",
        "The callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "some",
        "some",
        "The callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "forEach",
        "forEach",
        "The callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "flatMap",
        "flatMap",
        "Callbacks must return arrays; the callback type accepts (value, index), with no array parameter or thisArg, and sparse-array holes are visited.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "reduce",
        "reduce",
        "An initial value is required before the callback; the callback type omits the array parameter.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "reduceRight",
        "reduceRight",
        "An initial value is required before the callback; the callback type omits the array parameter.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "find",
        "findFirst",
        "Returns Option, with None replacing undefined; the callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "findLast",
        "findLast",
        "Returns Option, with None replacing undefined; the callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "findIndex",
        "findFirstIndex",
        "Returns Option<number>, with None replacing -1; the callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "findLastIndex",
        "findLastIndex",
        "Returns Option<number>, with None replacing -1; the callback type accepts (value, index), with no array parameter or thisArg.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "join",
        "join",
        "Accepts strings and requires an explicit separator.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "toReversed",
        "reverse",
        "Returns a new array, like native toReversed.",
    ),
    array_method_preference(
        ArrayMemberKind::InstanceMember,
        "toSorted",
        "sort",
        "Requires an explicit Order instead of native default ordering or a numeric comparator.",
    ),
    array_method_preference(
        ArrayMemberKind::ConstructorMember,
        "isArray",
        "isArray",
        "Narrows elements to unknown rather than any.",
    ),
];

// go: internal/rules/prefer_effect_array.go:54 arrayMethodNames 9dc072e177e223ab
//
// Rust: Go's `map[string]bool` of true values is a set.
pub(crate) static ARRAY_METHOD_NAMES: LazyLock<FxHashSet<&'static str>> = LazyLock::new(|| {
    let mut names = FxHashSet::default();
    for preference in ARRAY_METHOD_PREFERENCES {
        names.insert(preference.native_name);
    }
    names
});

impl ArrayMethodPreference {
    // go: internal/rules/prefer_effect_array.go:62 arrayMethodPreference.matches 89749b24ca5f7257
    pub(crate) fn matches(&self, tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
        if self.kind == ArrayMemberKind::ConstructorMember {
            return tp.is_native_array_constructor_method_reference(node, self.native_name);
        }
        tp.is_native_array_method_reference(node, self.native_name)
    }
}

// go: internal/rules/prefer_effect_array.go:69 runPreferEffectArray ade1bdb7a86f1df3
pub(crate) fn run_prefer_effect_array(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    let mut diagnostics = Vec::new();
    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        ctx: &mut Context<'_, '_>,
        node: P<Node>,
        diagnostics: &mut Vec<P<Diagnostic>>,
    ) -> bool {
        if ast::is_type_node(node) {
            return false;
        }
        let mut name: Option<P<Node>> = None;
        let mut receiver: Option<P<Node>> = None;
        match node.kind() {
            Kind::PropertyAccessExpression => {
                name = Some(node.as_property_access_expression().name);
                receiver = Some(node.as_property_access_expression().expression);
            }
            Kind::ElementAccessExpression => {
                let access = node.as_element_access_expression();
                if ast::is_string_literal_like(access.argument_expression) {
                    name = Some(access.argument_expression);
                    receiver = Some(access.expression);
                }
            }
            _ => {}
        }
        let mut parent = node.parent();
        while let Some(p) = parent
            && p.kind() == Kind::ParenthesizedExpression
        {
            parent = p.parent();
        }
        if let Some(name) = name
            && ARRAY_METHOD_NAMES.contains(name.text())
            && ast::skip_parentheses(receiver.unwrap()).kind() != Kind::SuperKeyword
            && !ast::is_assignment_target(node)
            && !ast::is_declaration_name(node)
            && parent.is_none_or(|parent| parent.kind() != Kind::DeleteExpression)
        {
            for preference in ARRAY_METHOD_PREFERENCES {
                if preference.matches(ctx.type_parser, node) {
                    diagnostics.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(name),
                        &tsdiag::CONSIDER_EFFECT_SLASH_ARRAY_0_INSTEAD_OF_NATIVE_ARRAY_1_2_EFFECT_PREFER_EFFECT_ARRAY,
                        &[],
                        &[preference.effect_api, preference.native_name, preference.caveat],
                    ));
                    break;
                }
            }
        }
        node.for_each_child(&mut |child| walk(ctx, child, diagnostics));
        false
    }
    let source_file = ctx.source_file;
    walk(ctx, source_file.as_node(), &mut diagnostics);
    diagnostics
}
