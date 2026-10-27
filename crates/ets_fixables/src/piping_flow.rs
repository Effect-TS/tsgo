use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, Node, NodeFactory, NodeFlags, SourceFile};
use ets_shim::core::{P, TextRange};
use ets_shim::scanner;
use ets_typeparser::{PipingFlowTransformation, TransformationKind};

// go: internal/fixables/piping_flow.go:18 removePipingFlowTransformation 740eee2d111ca71e
// removePipingFlowTransformation removes one step from its piping flow and
// leaves the step's input in place. A pipe argument is deleted, a pipe call left
// without steps collapses to its subject, and a data-first, data-last or unary
// application is replaced by its subject, parenthesized when the subject binds
// less tightly than the call it replaces. It edits nothing and returns false
// when the step's shape is not recognized, or when the subject would newly start
// an expression statement with a token that continues the previous statement.
//
// Rust: Go's nil tracker check has no counterpart (`&mut Tracker`); a transformation's callee is never nil.
// tsrs's call arguments are never nil, so those checks drop.
pub(crate) fn remove_piping_flow_transformation(
    tracker: &mut Tracker,
    sf: Option<P<SourceFile>>,
    transformation: Option<&PipingFlowTransformation>,
) -> bool {
    let (Some(sf), Some(transformation)) = (sf, transformation) else {
        return false;
    };

    match transformation.kind() {
        TransformationKind::DataFirst | TransformationKind::DataLast | TransformationKind::Call => {
            let mut call_node = direct_call_for_callee(Some(transformation.callee()));
            if transformation.kind() == TransformationKind::Call && transformation.args().len() > 0
            {
                // A curried step `factory(args)(subject)` is applied by the outer call.
                call_node = direct_call_for_callee(call_node);
            }
            let Some(call_node) = call_node else {
                return false;
            };
            let call = call_node.as_call_expression();
            let arguments = call.arguments().nodes();
            let mut subject = None;
            match transformation.kind() {
                TransformationKind::DataFirst => {
                    if arguments.len() as i64 == transformation.args().len() + 1 {
                        subject = Some(arguments[0]);
                    }
                }
                TransformationKind::DataLast => {
                    if arguments.len() as i64 == transformation.args().len() + 1 {
                        subject = Some(arguments[arguments.len() - 1]);
                    }
                }
                _ => {
                    if arguments.len() == 1 {
                        subject = Some(arguments[0]);
                    }
                }
            }
            let Some(subject) = subject else {
                return false;
            };
            let Some(replacement) = subject_in_call_position(tracker, sf, call_node, subject)
            else {
                return false;
            };
            tracker.replace_node(sf, Some(call_node), Some(replacement), None);
            true
        }

        TransformationKind::Pipe
        | TransformationKind::Pipeable
        | TransformationKind::EffectFn
        | TransformationKind::EffectFnUntraced => {
            let (Some(call_node), argument_index) =
                containing_call_argument(Some(transformation.callee()))
            else {
                return false;
            };
            if argument_index < 0 {
                return false;
            }
            let call = call_node.as_call_expression();
            let arguments = call.arguments().nodes();
            // Effect.fn pipelines always keep their body argument, so they never collapse.
            let mut minimum_arguments = 0;
            match transformation.kind() {
                TransformationKind::Pipeable => minimum_arguments = 1,
                TransformationKind::Pipe => minimum_arguments = 2, // Function.pipe also contains its subject.
                _ => {}
            }
            if arguments.len() == minimum_arguments {
                let mut subject = None;
                if transformation.kind() == TransformationKind::Pipe {
                    subject = Some(arguments[0]);
                } else if call.expression().kind() == Kind::PropertyAccessExpression {
                    subject = Some(call.expression().as_property_access_expression().expression());
                }
                let Some(subject) = subject else {
                    return false;
                };
                let Some(replacement) = subject_in_call_position(tracker, sf, call_node, subject)
                else {
                    return false;
                };
                tracker.replace_node(sf, Some(call_node), Some(replacement), None);
                return true;
            }

            let argument_index = argument_index as usize;
            if argument_index < arguments.len() - 1 {
                let start = scanner::get_token_pos_of_node(arguments[argument_index], sf, false);
                let end = scanner::get_token_pos_of_node(arguments[argument_index + 1], sf, false);
                tracker.delete_range(sf, TextRange::new(start, end));
                return true;
            }
            if argument_index > 0 {
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(
                        arguments[argument_index - 1].end(),
                        arguments[argument_index].end(),
                    ),
                );
                return true;
            }
            false
        }
    }
}

// go: internal/fixables/piping_flow.go:120 subjectInCallPosition e5233094a4202b7d
// subjectInCallPosition clones a removed step's subject for the position of the
// call it replaces. A call binds as tightly as any expression, so a subject that
// binds less tightly (a conditional, binary, assignment, `as` or arrow
// expression, an argument-less `new X`, or an optional chain that would extend
// over a following member access) is parenthesized unless the position accepts
// any expression. Object, function and class expressions are always
// parenthesized because they could start an expression statement.
//
// It returns nil when the replacement would newly begin an expression statement
// with a token that continues the previous statement when that one has no
// semicolon, for example `(cond ? a : b).pipe(...)` on the line after
// `const x = y`. The fix is then withheld instead of adding a semicolon,
// whether or not the previous statement already ends with one.
pub(crate) fn subject_in_call_position(
    tracker: &NodeFactory,
    sf: P<SourceFile>,
    position: P<Node>,
    subject: P<Node>,
) -> Option<P<Node>> {
    let parenthesize = subject_needs_parentheses(position, subject);
    let mut leading = Kind::OpenParenToken;
    if !parenthesize {
        leading = leading_token_kind(subject);
    }
    if continues_previous_statement(leading)
        && leading != leading_token_kind(position)
        && starts_expression_statement(sf, position)
    {
        return None;
    }
    let clone = tracker.deep_clone_node(Some(subject)).unwrap();
    if parenthesize {
        return Some(tracker.new_parenthesized_expression(clone));
    }
    Some(clone)
}

// go: internal/fixables/piping_flow.go:139 startsExpressionStatement 7127d9db26afe89c
// startsExpressionStatement reports whether node begins the expression
// statement that contains it.
pub(crate) fn starts_expression_statement(sf: P<SourceFile>, node: P<Node>) -> bool {
    let start = scanner::get_token_pos_of_node(node, sf, false);
    let mut parent = node.parent();
    while let Some(p) = parent {
        if scanner::get_token_pos_of_node(p, sf, false) != start {
            return false;
        }
        if p.kind() == Kind::ExpressionStatement {
            return true;
        }
        parent = p.parent();
    }
    false
}

// go: internal/fixables/piping_flow.go:154 leadingTokenKind b52c73b5e69322ec
// leadingTokenKind returns the kind of an expression's first token, as far as
// continuesPreviousStatement needs to tell tokens apart.
//
// Rust: the node is never nil (every step reads a non-optional child), so Go's trailing `KindUnknown` drops.
pub(crate) fn leading_token_kind(node: P<Node>) -> Kind {
    let mut node = node;
    loop {
        node = match node.kind() {
            Kind::ParenthesizedExpression => return Kind::OpenParenToken,
            Kind::ArrayLiteralExpression => return Kind::OpenBracketToken,
            Kind::NoSubstitutionTemplateLiteral | Kind::TemplateExpression => {
                return Kind::NoSubstitutionTemplateLiteral;
            }
            Kind::RegularExpressionLiteral => return Kind::SlashToken,
            Kind::TypeAssertionExpression => return Kind::LessThanToken,
            Kind::PrefixUnaryExpression => return node.as_prefix_unary_expression().operator(),
            Kind::PropertyAccessExpression => node.as_property_access_expression().expression(),
            Kind::ElementAccessExpression => node.as_element_access_expression().expression(),
            Kind::CallExpression => node.as_call_expression().expression(),
            Kind::TaggedTemplateExpression => node.as_tagged_template_expression().tag(),
            Kind::NonNullExpression => node.as_non_null_expression().expression(),
            Kind::BinaryExpression => node.as_binary_expression().left(),
            Kind::ConditionalExpression => node.as_conditional_expression().condition(),
            Kind::AsExpression => node.as_as_expression().expression(),
            Kind::SatisfiesExpression => node.as_satisfies_expression().expression(),
            Kind::PostfixUnaryExpression => node.as_postfix_unary_expression().operand(),
            kind => return kind,
        };
    }
}

// go: internal/fixables/piping_flow.go:198 continuesPreviousStatement 74e9e763cff61c0a
// continuesPreviousStatement reports whether a statement beginning with token
// is parsed as a continuation of a previous statement that has no semicolon.
pub(crate) fn continues_previous_statement(token: Kind) -> bool {
    matches!(
        token,
        Kind::OpenParenToken
            | Kind::OpenBracketToken
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::PlusToken
            | Kind::MinusToken
            | Kind::SlashToken
            | Kind::LessThanToken
    )
}

// go: internal/fixables/piping_flow.go:207 subjectNeedsParentheses 554d1044c7be3351
pub(crate) fn subject_needs_parentheses(position: P<Node>, subject: P<Node>) -> bool {
    match subject.kind() {
        Kind::ObjectLiteralExpression | Kind::FunctionExpression | Kind::ClassExpression => {
            return true;
        }
        _ => {}
    }
    if position_accepts_any_expression(position) {
        return false;
    }
    if subject.kind() == Kind::NewExpression {
        return subject.as_new_expression().arguments().is_none();
    }
    if subject.flags().intersects(NodeFlags::OptionalChain) {
        return true;
    }
    !ast::is_left_hand_side_expression(subject)
}

// go: internal/fixables/piping_flow.go:226 positionAcceptsAnyExpression 6a42cd0616c907ab
// positionAcceptsAnyExpression reports whether an expression of assignment
// precedence can stand at position without parentheses.
pub(crate) fn position_accepts_any_expression(position: P<Node>) -> bool {
    let Some(parent) = position.parent() else {
        return false;
    };
    match parent.kind() {
        Kind::CallExpression => parent.as_call_expression().expression() != position,
        Kind::NewExpression => parent.as_new_expression().expression() != position,
        Kind::ParenthesizedExpression
        | Kind::ArrayLiteralExpression
        | Kind::SpreadElement
        | Kind::TemplateSpan
        | Kind::ReturnStatement => true,
        Kind::VariableDeclaration => {
            parent.as_variable_declaration().initializer() == Some(position)
        }
        Kind::PropertyAssignment => parent.as_property_assignment().initializer() == position,
        _ => false,
    }
}

// go: internal/fixables/piping_flow.go:247 directCallForCallee 26fc1f018d838342
//
// Rust: returns the call node (Go: the node and its `*ast.CallExpression`, read with `as_call_expression()`).
pub(crate) fn direct_call_for_callee(callee: Option<P<Node>>) -> Option<P<Node>> {
    let callee = callee?;
    let parent = callee.parent()?;
    if parent.kind() != Kind::CallExpression {
        return None;
    }
    let call = parent.as_call_expression();
    if call.expression() != callee {
        return None;
    }
    Some(parent)
}

// go: internal/fixables/piping_flow.go:258 containingCallArgument 87b76417be1acb0b
//
// Rust: returns the call node and the argument index (Go also returns the node's `*ast.CallExpression`).
pub(crate) fn containing_call_argument(node: Option<P<Node>>) -> (Option<P<Node>>, i64) {
    let mut child = node;
    while let Some(c) = child {
        let Some(parent) = c.parent() else {
            break;
        };
        if parent.kind() == Kind::CallExpression {
            let call = parent.as_call_expression();
            for (index, &argument) in call.arguments().nodes().iter().enumerate() {
                if argument == c {
                    return (Some(parent), index as i64);
                }
            }
        }
        child = Some(parent);
    }
    (None, -1)
}
