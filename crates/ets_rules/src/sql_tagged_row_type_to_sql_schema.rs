use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THE_SQL_TAGGED_TEMPLATE_ONLY_ASSERTS_ITS_ROW_TYPE_0_AND_DOES_NOT_VALIDATE_IT_AT_RUNTIME_USE_SQL_SCHEMA_FIND_ALL_FIND_ONE_FIND_ONE_OPTION_FIND_NON_EMPTY_VOID_WITH_A_SCHEMA_FOR_RESULT_TO_DECODE_ROWS_EFFECT_SQL_TAGGED_ROW_TYPE_TO_SQL_SCHEMA;

// go: internal/rules/sql_tagged_row_type_to_sql_schema.go:10 SqlTaggedRowTypeToSqlSchema 03457c3c6f3acbdc
pub static SQL_TAGGED_ROW_TYPE_TO_SQL_SCHEMA: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "sqlTaggedRowTypeToSqlSchema",
    group: "effectNative",
    description: "Suggests SqlSchema instead of an explicit row type argument on Effect's sql tagged template",
    default_severity: Severity::Off,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, node: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if node.kind() == Kind::TaggedTemplateExpression {
                let tagged = node.as_tagged_template_expression();
                if let Some(type_arguments) = tagged.type_arguments()
                    && !type_arguments.nodes().is_empty()
                    && ctx.type_parser.is_effect_sql_statement_constructor_tagged_template(node)
                {
                    let c = ctx.checker();
                    let t = c.get_type_from_type_node_exported(type_arguments.nodes()[0]);
                    let row_type = c.type_to_string_exported(t);
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(tagged.tag()),
                        MESSAGE,
                        &[],
                        &[&row_type],
                    ));
                }
            }

            // ets:ast-walk finds tagged template expressions, which are not calls and never become piping-flow steps
            node.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        walk(ctx, ctx.source_file.as_node(), &mut diags);
        diags
    },
});
