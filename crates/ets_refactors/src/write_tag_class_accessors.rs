use std::sync::LazyLock;

use ets_refactor::{Context, Refactor};
use ets_rewriter::Tracker;
use ets_shim::ast::{Kind, Node, SourceFile, Symbol};
use ets_shim::astnav;
use ets_shim::checker::{Signature, SignatureKind, Type, TypeFlags, TypeFormatFlags};
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_shim::scanner;
use ets_typeparser::{EffectMajorVersion, TypeParser};
use rustc_hash::FxHashSet;

use crate::*;

// go: internal/refactors/write_tag_class_accessors.go:17 WriteTagClassAccessors
pub static WRITE_TAG_CLASS_ACCESSORS: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "writeTagClassAccessors",
    description: "Implement Service accessors",
    kind: "rewrite.effect.writeTagClassAccessors",
    run: run_write_tag_class_accessors,
});

// go: internal/refactors/write_tag_class_accessors.go:25 involvedMember
// involvedMember holds a property symbol and its type for accessor generation.
#[derive(Clone, Copy)]
pub(crate) struct InvolvedMember {
    pub(crate) property: P<Symbol>,
    pub(crate) property_type: P<Type>,
}

// go: internal/refactors/write_tag_class_accessors.go:30 runWriteTagClassAccessors
// Rust: the edit closure uses the type parser, so the action is built with `new_refactor_action_with_type_parser`
// (Go's closure captures `ctx` and `c`).
pub(crate) fn run_write_tag_class_accessors(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // V3-only refactor
    if ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4 {
        return Vec::new();
    }

    // Walk ancestors to find a ClassDeclaration
    let class_node = std::iter::successors(Some(token), |n| n.parent())
        .take_while(|node| node.kind() != Kind::SourceFile)
        .find(|node| node.kind() == Kind::ClassDeclaration);
    let Some(class_node) = class_node else {
        return Vec::new();
    };

    // Check ExtendsEffectService (with accessors: true) or ExtendsEffectTag
    let class_name;
    if let Some(service_result) = ctx.type_parser.extends_effect_v3_service(class_node) {
        if !has_accessors_true(service_result.options()) {
            return Vec::new();
        }
        class_name = service_result.class_name();
    } else if let Some(tag_result) = ctx.type_parser.extends_effect_tag(class_node) {
        // Effect.Tag always has accessors
        class_name = tag_result.class_name();
    } else {
        return Vec::new();
    }

    // Resolve the service shape type via the _Service variance struct
    let Some(class_symbol) = ctx.type_parser.get_symbol_at_location(class_name) else {
        return Vec::new();
    };
    let Some(class_type) =
        ctx.checker().get_type_of_symbol_at_location(class_symbol, Some(class_node))
    else {
        return Vec::new();
    };

    let Some(context_tag) = ctx.type_parser.context_tag(class_type) else {
        return Vec::new();
    };

    // Unroll union and filter out primitives
    let non_primitive_services = filter_non_primitive_types(
        &ctx.type_parser.unroll_union_members(context_tag.shape()).unwrap_or_default(),
    );
    if non_primitive_services.is_empty() {
        return Vec::new();
    }

    // Find involved members (those with generics or multiple overloads)
    let mut members = Vec::new();
    let mut seen = FxHashSet::default();
    for &service in &non_primitive_services {
        for &property in ctx.checker().get_properties_of_type_exported(service) {
            let prop_name = property.name();
            if seen.contains(prop_name) {
                continue;
            }
            // Go: `GetTypeOfSymbolAtLocation` never returns nil for a property.
            let Some(property_type) =
                ctx.checker().get_type_of_symbol_at_location(property, Some(class_node))
            else {
                continue;
            };
            let call_signatures =
                ctx.checker().get_signatures_of_type_exported(property_type, SignatureKind::Call);
            if call_signatures.is_empty() {
                continue;
            }
            let has_generics = call_signatures.iter().any(|sig| !sig.type_parameters().is_empty());
            if call_signatures.len() > 1 || has_generics {
                members.push(InvolvedMember { property, property_type });
                seen.insert(prop_name);
            }
        }
    }

    if members.is_empty() {
        return Vec::new();
    }

    let effect_identifier = ets_typeparser::find_effect_module_identifier(ctx.source_file);
    let class_name_text = scanner::get_text_of_node(class_name);

    let sf = ctx.source_file;
    let Some(mut action) = ctx.new_refactor_action_with_type_parser(
        "Implement Service accessors".to_string(),
        |tracker, tp| {
            generate_accessors(
                tracker,
                sf,
                tp,
                class_node,
                &class_name_text,
                &effect_identifier,
                &members,
            );
        },
    ) else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.writeTagClassAccessors");
    vec![action]
}

// go: internal/refactors/write_tag_class_accessors.go:147 hasAccessorsTrue
// hasAccessorsTrue checks if an options object literal has `accessors: true`.
pub(crate) fn has_accessors_true(options: Option<P<Node>>) -> bool {
    let Some(options) = options.filter(|o| o.kind() == Kind::ObjectLiteralExpression) else {
        return false;
    };
    let obj_lit = options.as_object_literal_expression();
    for &prop in obj_lit.properties().nodes() {
        if prop.kind() != Kind::PropertyAssignment {
            continue;
        }
        let pa = prop.as_property_assignment();
        let name = scanner::get_text_of_node(pa.name());
        if name == "accessors" && pa.initializer().kind() == Kind::TrueKeyword {
            return true;
        }
    }
    false
}

// go: internal/refactors/write_tag_class_accessors.go:172 filterNonPrimitiveTypes
// filterNonPrimitiveTypes filters out primitive types (number, string, boolean, literals).
pub(crate) fn filter_non_primitive_types(types: &[P<Type>]) -> Vec<P<Type>> {
    let mut result = Vec::new();
    for &t in types {
        let flags = t.flags();
        if flags.intersects(TypeFlags::Number)
            || flags.intersects(TypeFlags::String)
            || flags.intersects(TypeFlags::Boolean)
            || flags.intersects(TypeFlags::Literal)
        {
            continue;
        }
        result.push(t);
    }
    result
}

// go: internal/refactors/write_tag_class_accessors.go:188 generateAccessors
// generateAccessors generates accessor text directly via InsertText.
//
// Rust: Go's `ctx` and `c` are the source file and the parser (`tp.checker()`): the context's parser is lent to
// the edit closure.
pub(crate) fn generate_accessors(
    tracker: &mut Tracker,
    source_file: P<SourceFile>,
    tp: &mut TypeParser<'_>,
    class_node: P<Node>,
    class_name_text: &str,
    effect_identifier: &str,
    members: &[InvolvedMember],
) {
    let class_decl = class_node.as_class_declaration();

    // Find insert location: first member's pos, or end of class - 1
    let mut insert_pos = class_node.end().expect_source("a parsed node") - 1;
    if let Some(&first) = class_decl.members().nodes().first() {
        insert_pos = first.pos().expect_source("a parsed node");
    }
    // Go: `ctx.BytePosToLSPPosition(int(insertPos))`.
    let (line, character) =
        scanner::get_ecma_line_and_utf16_character_of_position(&*source_file, insert_pos);
    let ls_pos = lsproto::Position { line: u32::try_from(line).unwrap(), character };

    let mut sb = String::new();
    for member in members {
        let property_name = member.property.name();
        let call_signatures =
            tp.checker().get_signatures_of_type_exported(member.property_type, SignatureKind::Call);
        if call_signatures.is_empty() {
            continue;
        }

        let force_any = call_signatures.len() > 1;

        // Build signature type strings
        let mut sig_type_strs = Vec::new();
        for &sig in call_signatures {
            let sig_str =
                build_proxy_signature_text(tp, sig, class_node, class_name_text, effect_identifier);
            if !sig_str.is_empty() {
                sig_type_strs.push(sig_str);
            }
        }
        if sig_type_strs.is_empty() {
            continue;
        }

        // Build type annotation
        let type_annotation = if sig_type_strs.len() == 1 {
            sig_type_strs[0].clone()
        } else {
            "(".to_string() + &sig_type_strs.join(") & (") + ")"
        };

        // Build the property text
        sb.push_str("\n  static override ");
        sb.push_str(property_name);
        sb.push_str(": ");
        sb.push_str(&type_annotation);
        sb.push_str(" = (");
        if force_any {
            sb.push_str("...args: any[]");
        } else {
            sb.push_str("...args");
        }
        sb.push_str(") => ");
        sb.push_str(effect_identifier);
        sb.push_str(".andThen(");
        sb.push_str(class_name_text);
        sb.push_str(", (");
        if force_any {
            sb.push_str("_: any");
        } else {
            sb.push('_');
        }
        sb.push_str(") => _.");
        sb.push_str(property_name);
        sb.push_str("(...args))");
        if force_any {
            sb.push_str(" as any");
        }
    }

    if !sb.is_empty() {
        tracker.insert_text(source_file, ls_pos, &(sb + "\n"));
    }
}

// go: internal/refactors/write_tag_class_accessors.go:274 buildProxySignatureText
// buildProxySignatureText builds the text representation of a proxy function type for a call signature.
//
// Rust: Go's `c` is `tp.checker()`.
pub(crate) fn build_proxy_signature_text(
    tp: &mut TypeParser<'_>,
    sig: P<Signature>,
    class_node: P<Node>,
    class_name_text: &str,
    effect_identifier: &str,
) -> String {
    let mut sb = String::new();

    // Type parameters
    let tps = sig.type_parameters();
    if !tps.is_empty() {
        sb.push('<');
        for (i, &type_param) in tps.iter().enumerate() {
            if i > 0 {
                sb.push_str(", ");
            }
            sb.push_str(type_param.symbol().expect("a type parameter has a symbol").name());
            let constraint_type =
                tp.checker().get_constraint_of_type_parameter_exported(type_param);
            if let Some(constraint_type) = constraint_type {
                sb.push_str(" extends ");
                sb.push_str(&tp.checker().type_to_string_ex_exported(
                    constraint_type,
                    Some(class_node),
                    TypeFormatFlags::NoTruncation,
                    None,
                ));
            }
        }
        sb.push('>');
    }

    // Parameters
    sb.push('(');
    let params = sig.parameters();
    for (i, &param) in params.iter().enumerate() {
        if i > 0 {
            sb.push_str(", ");
        }
        if sig.has_rest_parameter() && i == params.len() - 1 {
            sb.push_str("...");
        }
        sb.push_str(param.name());
        sb.push_str(": ");
        // Go: `GetTypeOfSymbolAtLocation` never returns nil for a parameter.
        let param_type = tp
            .checker()
            .get_type_of_symbol_at_location(param, Some(class_node))
            .expect("a parameter has a type");
        sb.push_str(&tp.checker().type_to_string_ex_exported(
            param_type,
            Some(class_node),
            TypeFormatFlags::NoTruncation,
            None,
        ));
    }
    sb.push(')');

    // Return type
    sb.push_str(" => ");
    let return_type = tp.checker().get_return_type_of_signature_exported(sig);
    let wrapped_return = build_wrapped_return_type_text(
        tp,
        return_type,
        class_node,
        class_name_text,
        effect_identifier,
    );
    sb.push_str(&wrapped_return);

    sb
}

// go: internal/refactors/write_tag_class_accessors.go:328 buildWrappedReturnTypeText
// buildWrappedReturnTypeText builds the text representation of the wrapped return type.
//
// Rust: Go's `c` is `tp.checker()`.
pub(crate) fn build_wrapped_return_type_text(
    tp: &mut TypeParser<'_>,
    return_type: P<Type>,
    class_node: P<Node>,
    class_name_text: &str,
    effect_identifier: &str,
) -> String {
    let flags = TypeFormatFlags::NoTruncation;

    // Try to parse as Effect type
    if let Some(effect) = tp.effect_type(return_type) {
        let a_str =
            tp.checker().type_to_string_ex_exported(effect.a(), Some(class_node), flags, None);
        let e_str =
            tp.checker().type_to_string_ex_exported(effect.e(), Some(class_node), flags, None);

        let r_str = if effect.r().flags().intersects(TypeFlags::Never) {
            class_name_text.to_string()
        } else {
            class_name_text.to_string()
                + " | "
                + &tp.checker().type_to_string_ex_exported(
                    effect.r(),
                    Some(class_node),
                    flags,
                    None,
                )
        };

        return effect_identifier.to_string()
            + ".Effect<"
            + &a_str
            + ", "
            + &e_str
            + ", "
            + &r_str
            + ">";
    }

    // Try to detect Promise<T>
    let return_type_str =
        tp.checker().type_to_string_ex_exported(return_type, Some(class_node), flags, None);
    if let Some(inner_type_str) = is_promise_type_string(&return_type_str) {
        return effect_identifier.to_string()
            + ".Effect<"
            + inner_type_str
            + ", Cause.UnknownException, "
            + class_name_text
            + ">";
    }

    // Fallback: Effect<A, never, ClassName>
    let a_str = tp.checker().type_to_string_ex_exported(return_type, Some(class_node), flags, None);
    effect_identifier.to_string() + ".Effect<" + &a_str + ", never, " + class_name_text + ">"
}

// go: internal/refactors/write_tag_class_accessors.go:364 isPromiseTypeString
// isPromiseTypeString checks if the type string represents a Promise<T> and extracts the inner type string.
//
// Rust: Go's `(inner, ok)` is `Option<&str>`.
pub(crate) fn is_promise_type_string(type_str: &str) -> Option<&str> {
    if !type_str.starts_with("Promise<") {
        return None;
    }
    let bytes = type_str.as_bytes();
    let mut depth = 0;
    for (i, &b) in bytes.iter().enumerate().skip(8) {
        match b {
            b'<' => depth += 1,
            b'>' => {
                if depth == 0 {
                    return Some(&type_str[8..i]);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}
