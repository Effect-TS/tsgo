use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectMajorVersion;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::X_0_EXECUTES_SYNCHRONOUSLY_USE_SCHEMA_1_TO_COMPOSE_THIS_OPERATION_THROUGH_EFFECT_WITHOUT_THROWING_EFFECT_SCHEMA_SYNC;

// go: internal/rules/schema_sync.go:13 SchemaSync a406a27baaa191b2
// SchemaSync discourages synchronous Schema decoding and encoding in any context.
pub static SCHEMA_SYNC: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaSync",
    group: "effectNative",
    description: "Suggests Effect-based Schema decoding and encoding instead of synchronous methods",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut methods = SYNC_TO_EFFECT_METHOD_V3;
        if ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4 {
            methods = SYNC_TO_EFFECT_METHOD_V4;
        }

        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            node: P<Node>,
            methods: SyncToEffectMethod,
            diags: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if node.kind() == Kind::CallExpression {
                let callee = node.as_call_expression().expression();
                let mut reference = ast::skip_parentheses(callee);
                if reference.kind() == Kind::ElementAccessExpression {
                    reference = reference.as_element_access_expression().argument_expression();
                }
                for &(sync_name, effect_name) in methods {
                    if ctx
                        .type_parser
                        .is_node_reference_to_effect_schema_module_api(reference, sync_name)
                        || ctx.type_parser.is_node_reference_to_effect_parse_result_module_api(
                            reference, sync_name,
                        )
                        || ctx.type_parser.is_node_reference_to_effect_schema_parser_module_api(
                            reference, sync_name,
                        )
                    {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ctx.get_error_range(callee),
                            MESSAGE,
                            &[],
                            &[
                                &ets_shim::scanner::get_source_text_of_node_from_source_file(
                                    ctx.source_file,
                                    callee,
                                    false,
                                ),
                                effect_name,
                            ],
                        ));
                        break;
                    }
                }
            }
            node.for_each_child(&mut |child| walk(ctx, child, methods, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, methods, &mut diags);
        diags
    },
});
