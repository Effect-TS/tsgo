use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::diagnostics as tsdiag;
use rustc_hash::FxHashSet;

// go: internal/rules/prefer_effect_string.go:10 PreferEffectString 411678490545732d
pub static PREFER_EFFECT_STRING: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "preferEffectString",
        group: "effectNative",
        description: "Suggests Effect String APIs for native String method invocations, with semantic caveats",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::CONSIDER_EFFECT_SLASH_STRING_0_INSTEAD_OF_NATIVE_STRING_1_2_EFFECT_PREFER_EFFECT_STRING.code()],
        run: run_prefer_effect_string,
    }
});

// go: internal/rules/prefer_effect_string.go:20 stringMethodPreference 8c13423f67ffd4b2
pub(crate) struct StringMethodPreference {
    pub(crate) native_name: &'static str,
    pub(crate) effect_api: &'static str,
    pub(crate) caveat: &'static str,
}

/// Rust-only: Go's positional struct literal `{nativeName, effectAPI, caveat}`.
const fn string_method_preference(
    native_name: &'static str,
    effect_api: &'static str,
    caveat: &'static str,
) -> StringMethodPreference {
    StringMethodPreference { native_name, effect_api, caveat }
}

// go: internal/rules/prefer_effect_string.go:26 stringMethodPreferences f17fa632185a92dc
pub(crate) static STRING_METHOD_PREFERENCES: &[StringMethodPreference] = &[
    string_method_preference("at", "at", "Returns Option, with None replacing undefined."),
    string_method_preference(
        "charAt",
        "charAt",
        "Returns Option, with None replacing the empty string for an out-of-range index.",
    ),
    string_method_preference(
        "charCodeAt",
        "charCodeAt",
        "Returns Option<number>, with None replacing NaN.",
    ),
    string_method_preference(
        "codePointAt",
        "codePointAt",
        "Returns Option<number>, with None replacing undefined.",
    ),
    string_method_preference(
        "concat",
        "concat",
        "Accepts exactly one string argument; native concat accepts any number of arguments and converts non-strings.",
    ),
    string_method_preference("endsWith", "endsWith", "Same runtime behavior."),
    string_method_preference("includes", "includes", "Same runtime behavior."),
    string_method_preference(
        "indexOf",
        "indexOf",
        "Returns Option<number>, with None replacing -1; there is no position parameter.",
    ),
    string_method_preference(
        "lastIndexOf",
        "lastIndexOf",
        "Returns Option<number>, with None replacing -1; there is no position parameter.",
    ),
    string_method_preference(
        "localeCompare",
        "localeCompare",
        "Returns an Ordering of -1, 0, or 1; Effect v4 types locales as Array<string>, so a single locale string needs an array.",
    ),
    string_method_preference("match", "match", "Returns Option, with None replacing null."),
    string_method_preference(
        "matchAll",
        "matchAll",
        "Requires a RegExp; string patterns are not accepted.",
    ),
    string_method_preference("normalize", "normalize", "Same runtime behavior."),
    string_method_preference("padEnd", "padEnd", "Same runtime behavior."),
    string_method_preference("padStart", "padStart", "Same runtime behavior."),
    string_method_preference(
        "repeat",
        "repeat",
        "In Effect v4, a negative count returns an empty string instead of throwing RangeError.",
    ),
    string_method_preference(
        "replace",
        "replace",
        "Requires a string replacement; replacer callbacks are not accepted.",
    ),
    string_method_preference(
        "replaceAll",
        "replaceAll",
        "Requires a string replacement; replacer callbacks are not accepted.",
    ),
    string_method_preference("search", "search", "Returns Option<number>, with None replacing -1."),
    string_method_preference("slice", "slice", "Same runtime behavior."),
    string_method_preference(
        "split",
        "split",
        "Returns a NonEmptyArray, so splitting an empty string by \"\" returns [\"\"] instead of []; there is no limit parameter.",
    ),
    string_method_preference("startsWith", "startsWith", "Same runtime behavior."),
    string_method_preference("substring", "substring", "Same runtime behavior."),
    string_method_preference(
        "toLocaleLowerCase",
        "toLocaleLowerCase",
        "Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale.",
    ),
    string_method_preference(
        "toLocaleUpperCase",
        "toLocaleUpperCase",
        "Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale.",
    ),
    string_method_preference(
        "toLowerCase",
        "toLowerCase",
        "Same runtime behavior; literal inputs keep a Lowercase<T> type.",
    ),
    string_method_preference(
        "toUpperCase",
        "toUpperCase",
        "Same runtime behavior; literal inputs keep an Uppercase<T> type.",
    ),
    string_method_preference(
        "trim",
        "trim",
        "Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r.",
    ),
    string_method_preference(
        "trimStart",
        "trimStart",
        "Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r.",
    ),
    string_method_preference(
        "trimEnd",
        "trimEnd",
        "Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r.",
    ),
    string_method_preference(
        "trimLeft",
        "trimStart",
        "Deprecated alias of trimStart; the literal result type only removes spaces, \\n, \\t, and \\r.",
    ),
    string_method_preference(
        "trimRight",
        "trimEnd",
        "Deprecated alias of trimEnd; the literal result type only removes spaces, \\n, \\t, and \\r.",
    ),
];

// go: internal/rules/prefer_effect_string.go:61 stringMethodNames a337baf7a722032e
//
// Rust: Go's `map[string]bool` of true values is a set.
pub(crate) static STRING_METHOD_NAMES: LazyLock<FxHashSet<&'static str>> = LazyLock::new(|| {
    let mut names = FxHashSet::default();
    for preference in STRING_METHOD_PREFERENCES {
        names.insert(preference.native_name);
    }
    names
});

// go: internal/rules/prefer_effect_string.go:69 runPreferEffectString 100ea1289cc96a57
pub(crate) fn run_prefer_effect_string(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
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
        if let Some(parent) = parent
            && parent.kind() == Kind::CallExpression
            && ast::skip_parentheses(parent.as_call_expression().expression) == node
            && let Some(name) = name
            && STRING_METHOD_NAMES.contains(name.text())
            && ast::skip_parentheses(receiver.unwrap()).kind() != Kind::SuperKeyword
        {
            for preference in STRING_METHOD_PREFERENCES {
                if ctx.type_parser.is_native_string_method_reference(node, preference.native_name) {
                    diagnostics.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(name),
                        &tsdiag::CONSIDER_EFFECT_SLASH_STRING_0_INSTEAD_OF_NATIVE_STRING_1_2_EFFECT_PREFER_EFFECT_STRING,
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
