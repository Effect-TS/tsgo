use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::Classes_extending_Schema_Opaque_must_not_declare_instance_members_effect_schemaOpaqueInstanceMember;

// go: internal/rules/schema_opaque_instance_member.go:10 SchemaOpaqueInstanceMember
pub static SCHEMA_OPAQUE_INSTANCE_MEMBER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaOpaqueInstanceMember",
    group: "correctness",
    description: "Disallows instance members in classes extending Schema.Opaque",
    default_severity: Severity::Error,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            node: P<Node>,
            diagnostics: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if (node.kind() == Kind::ClassDeclaration || node.kind() == Kind::ClassExpression)
                && ctx.type_parser.extends_schema_opaque(node).is_some()
            {
                for &member in node.members() {
                    if is_schema_opaque_instance_member(Some(member)) {
                        diagnostics.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ctx.get_error_range(member),
                            MESSAGE,
                            &[],
                            &[],
                        ));
                    }
                }
            }
            node.for_each_child(&mut |child| walk(ctx, child, diagnostics));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, &mut diagnostics);
        diagnostics
    },
});

// go: internal/rules/schema_opaque_instance_member.go:41 isSchemaOpaqueInstanceMember
pub(crate) fn is_schema_opaque_instance_member(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if ast::has_syntactic_modifier(node, ModifierFlags::Static) {
        return false;
    }
    matches!(
        node.kind(),
        Kind::PropertyDeclaration
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::Constructor
    )
}
