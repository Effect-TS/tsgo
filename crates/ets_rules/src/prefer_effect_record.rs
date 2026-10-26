use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;
use rustc_hash::FxHashSet;

// go: internal/rules/prefer_effect_record.go:11 PreferEffectRecord
pub static PREFER_EFFECT_RECORD: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferEffectRecord",
    group: "effectNative",
    description: "Suggests Effect Record APIs for native Object member references, with semantic caveats",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::CONSIDER_EFFECT_SLASH_RECORD_0_INSTEAD_OF_NATIVE_1_2_EFFECT_PREFER_EFFECT_RECORD
            .code(),
    ],
    run: run_prefer_effect_record,
});

// go: internal/rules/prefer_effect_record.go:21 recordMemberNamespace
// go: internal/rules/prefer_effect_record.go:23 recordInstanceMember
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RecordMemberNamespace {
    InstanceMember,
    ConstructorMember,
}

// go: internal/rules/prefer_effect_record.go:28 recordMethodPreference
pub(crate) struct RecordMethodPreference {
    pub(crate) namespace: RecordMemberNamespace,
    pub(crate) native_name: &'static str,
    pub(crate) effect_api: &'static str,
    pub(crate) caveat: &'static str,
}

/// Rust-only: Go's positional struct literal `{namespace, nativeName, effectAPI, caveat}`.
const fn record_method_preference(
    namespace: RecordMemberNamespace,
    native_name: &'static str,
    effect_api: &'static str,
    caveat: &'static str,
) -> RecordMethodPreference {
    RecordMethodPreference { namespace, native_name, effect_api, caveat }
}

// go: internal/rules/prefer_effect_record.go:35 recordMethodPreferences
pub(crate) static RECORD_METHOD_PREFERENCES: &[RecordMethodPreference] = &[
    record_method_preference(
        RecordMemberNamespace::ConstructorMember,
        "keys",
        "keys",
        "Requires a record rather than arrays or primitives; types keys as the inferred key union, which does not guarantee exact runtime keys.",
    ),
    record_method_preference(
        RecordMemberNamespace::ConstructorMember,
        "values",
        "values",
        "Requires a record rather than arrays or primitives; snapshots keys before reads, so getter or proxy mutations can change the result.",
    ),
    record_method_preference(
        RecordMemberNamespace::ConstructorMember,
        "entries",
        "toEntries",
        "Requires a record rather than arrays or primitives; types entries with inferred keys and snapshots keys before reads, so getter or proxy mutations can change the result.",
    ),
    record_method_preference(
        RecordMemberNamespace::ConstructorMember,
        "fromEntries",
        "fromEntries",
        "Requires typed string or symbol key tuples; native numeric keys and loose entry arrays need adaptation.",
    ),
    record_method_preference(
        RecordMemberNamespace::ConstructorMember,
        "hasOwn",
        "has",
        "Requires a record rather than arrays or primitives and a string or symbol key of that record's key type; arbitrary strings may need narrowing.",
    ),
    record_method_preference(
        RecordMemberNamespace::InstanceMember,
        "hasOwnProperty",
        "has",
        "Pass the receiver explicitly; requires a record rather than arrays or primitives and a string or symbol key of that record's key type. Avoids instance binding and overrides.",
    ),
];

// go: internal/rules/prefer_effect_record.go:44 recordMethodNames
//
// Rust: Go's `map[string]bool` of true values is a set.
pub(crate) static RECORD_METHOD_NAMES: LazyLock<FxHashSet<&'static str>> = LazyLock::new(|| {
    let mut names = FxHashSet::default();
    for preference in RECORD_METHOD_PREFERENCES {
        names.insert(preference.native_name);
    }
    names
});

impl RecordMethodPreference {
    // go: internal/rules/prefer_effect_record.go:52 recordMethodPreference.matches
    pub(crate) fn matches(&self, tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
        if self.namespace == RecordMemberNamespace::ConstructorMember {
            return tp.is_native_object_constructor_method_reference(node, self.native_name);
        }
        tp.is_native_object_method_reference(node, self.native_name)
    }

    // go: internal/rules/prefer_effect_record.go:59 recordMethodPreference.nativeLabel
    pub(crate) fn native_label(&self) -> String {
        if self.namespace == RecordMemberNamespace::InstanceMember {
            return format!("Object.prototype.{}", self.native_name);
        }
        format!("Object.{}", self.native_name)
    }
}

// go: internal/rules/prefer_effect_record.go:66 runPreferEffectRecord
pub(crate) fn run_prefer_effect_record(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
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
            && RECORD_METHOD_NAMES.contains(name.text())
            && ast::skip_parentheses(receiver.unwrap()).kind() != Kind::SuperKeyword
            && !ast::is_assignment_target(node)
            && !ast::is_declaration_name(node)
            && parent.is_none_or(|parent| parent.kind() != Kind::DeleteExpression)
        {
            for preference in RECORD_METHOD_PREFERENCES {
                if preference.matches(ctx.type_parser, node) {
                    diagnostics.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(name),
                        &tsdiag::CONSIDER_EFFECT_SLASH_RECORD_0_INSTEAD_OF_NATIVE_1_2_EFFECT_PREFER_EFFECT_RECORD,
                        &[],
                        &[preference.effect_api, &preference.native_label(), preference.caveat],
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
