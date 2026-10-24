use ets_readonly::Slice;
use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/returning_dispatch.go:9 DispatchConditionKind
// go: internal/typeparser/returning_dispatch.go:11 DispatchConditionPredicate
// DispatchConditionKind identifies how one result-producing branch is
// selected.
//
// Rust: Go's `uint8` enum with its two constants, the only values it takes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum DispatchConditionKind {
    // DispatchConditionPredicate is a boolean condition from an if or
    // conditional expression.
    #[default]
    Predicate = 0,
    // DispatchConditionSwitchCase compares Subject, the switch
    // discriminant, with Value, the case expression.
    SwitchCase = 1,
}

// go: internal/typeparser/returning_dispatch.go:26 DispatchCondition
// DispatchCondition preserves the source nodes that select one branch.
// Predicate conditions set Subject to the condition expression and leave Value
// nil. Switch cases set Subject to the shared discriminant and Value to the
// case expression. Source is the predicate expression or case clause used for
// diagnostics. TagSubject and TagValue are populated for `_tag` equality and
// Effect Predicate.isTagged conditions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DispatchCondition {
    pub(crate) kind: DispatchConditionKind,
    pub(crate) source: P<Node>,
    pub(crate) subject: P<Node>,
    pub(crate) value: Option<P<Node>>,
    pub(crate) tag_subject: Option<P<Node>>,
    pub(crate) tag_value: Option<P<Node>>,
}

impl DispatchCondition {
    // go: internal/typeparser/returning_dispatch.go:35 DispatchCondition.Kind
    pub fn kind(self) -> DispatchConditionKind {
        self.kind
    }

    // go: internal/typeparser/returning_dispatch.go:37 DispatchCondition.Source
    pub fn source(self) -> P<Node> {
        self.source
    }

    // go: internal/typeparser/returning_dispatch.go:39 DispatchCondition.Subject
    pub fn subject(self) -> P<Node> {
        self.subject
    }

    // go: internal/typeparser/returning_dispatch.go:41 DispatchCondition.Value
    pub fn value(self) -> Option<P<Node>> {
        self.value
    }

    // go: internal/typeparser/returning_dispatch.go:43 DispatchCondition.TagSubject
    pub fn tag_subject(self) -> Option<P<Node>> {
        self.tag_subject
    }

    // go: internal/typeparser/returning_dispatch.go:45 DispatchCondition.TagValue
    pub fn tag_value(self) -> Option<P<Node>> {
        self.tag_value
    }
}

// go: internal/typeparser/returning_dispatch.go:49 ResultDispatchBranch
// ResultDispatchBranch is one source-ordered branch in a decoded
// result-producing expression or block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResultDispatchBranch {
    pub(crate) condition: DispatchCondition,
    pub(crate) result: P<Node>,
}

impl ResultDispatchBranch {
    // go: internal/typeparser/returning_dispatch.go:54 ResultDispatchBranch.Condition
    pub fn condition(self) -> DispatchCondition {
        self.condition
    }

    // go: internal/typeparser/returning_dispatch.go:56 ResultDispatchBranch.Result
    pub fn result(self) -> P<Node> {
        self.result
    }
}

// go: internal/typeparser/returning_dispatch.go:60 ResultDispatch
// ResultDispatch is an ordered, first-match result dispatch with an
// optional fallback.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResultDispatch {
    pub(crate) node: P<Node>,
    pub(crate) branches: Vec<ResultDispatchBranch>,
    pub(crate) fallback: Option<P<Node>>,
}

impl ResultDispatch {
    // go: internal/typeparser/returning_dispatch.go:66 ResultDispatch.Node
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/returning_dispatch.go:68 ResultDispatch.Branches
    pub fn branches(&self) -> Slice<'_, ResultDispatchBranch> {
        ets_readonly::new(&self.branches[..])
    }

    // go: internal/typeparser/returning_dispatch.go:72 ResultDispatch.Fallback
    pub fn fallback(&self) -> Option<P<Node>> {
        self.fallback
    }
}

// go: internal/typeparser/returning_dispatch.go:76 ParsedReturningDispatch
// ParsedReturningDispatch is a decoded arrow function or function expression
// whose body consists entirely of result-producing branches.
//
// Rust: the parameters are the function's own parameter list (`None` where Go's slice is nil); Go's never-nil
// `*ResultDispatch` is held by value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedReturningDispatch {
    pub(crate) node: P<Node>,
    pub(crate) params: Option<&'static [P<Node>]>,
    pub(crate) body: P<Node>,
    pub(crate) dispatch: ResultDispatch,
}

impl ParsedReturningDispatch {
    // go: internal/typeparser/returning_dispatch.go:83 ParsedReturningDispatch.Node
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/returning_dispatch.go:85 ParsedReturningDispatch.Params
    pub fn params(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.params)
    }

    // go: internal/typeparser/returning_dispatch.go:87 ParsedReturningDispatch.Body
    pub fn body(&self) -> P<Node> {
        self.body
    }

    // go: internal/typeparser/returning_dispatch.go:89 ParsedReturningDispatch.Dispatch
    pub fn dispatch(&self) -> &ResultDispatch {
        &self.dispatch
    }
}

// go: internal/typeparser/returning_dispatch.go:91 resultDispatchSyntax
#[derive(Default)]
pub(crate) struct ResultDispatchSyntax {
    branches: Vec<ResultDispatchBranch>,
    fallback: Option<P<Node>>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/returning_dispatch.go:100 TypeParser.ParseResultDispatch
    // ParseResultDispatch decodes a result-producing conditional expression or
    // block into source-ordered branches and an optional fallback. False-arm
    // conditional chains are flattened; conditionals in a true arm are rejected.
    // Tag hints include checker-backed recognition of Effect Predicate.isTagged.
    pub fn parse_result_dispatch(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<ResultDispatch> {
        let node = node.into()?;
        let mut syntax = parse_result_dispatch_body(Some(node))?;
        if syntax.branches.is_empty() {
            return None;
        }
        for branch in &mut syntax.branches {
            let condition = &mut branch.condition;
            (condition.tag_subject, condition.tag_value) =
                self.dispatch_condition_tag_nodes(*condition);
        }
        Some(ResultDispatch { node, branches: syntax.branches, fallback: syntax.fallback })
    }

    // go: internal/typeparser/returning_dispatch.go:123 TypeParser.ParseReturningDispatch
    // ParseReturningDispatch decodes an arrow function or function expression into
    // source-ordered result branches and an optional fallback. It recognizes
    // conditional expressions, returned conditional expressions, if/else-if,
    // sequential returning if statements, and returning switch cases.
    pub fn parse_returning_dispatch(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<ParsedReturningDispatch> {
        let node = unwrap_result_dispatch_expression(node.into())?;
        if node.kind() != Kind::ArrowFunction && node.kind() != Kind::FunctionExpression {
            return None;
        }
        if get_function_like_type_parameters(node).is_some_and(|tps| !tps.nodes().is_empty()) {
            return None;
        }
        let body = get_function_like_body(node)?;

        let dispatch = self.parse_result_dispatch(body)?;
        let params = get_function_like_parameters(node).map(|parameters| parameters.nodes());
        Some(ParsedReturningDispatch { node, params, body, dispatch })
    }
}

// go: internal/typeparser/returning_dispatch.go:154 parseResultDispatchBody
pub(crate) fn parse_result_dispatch_body(node: Option<P<Node>>) -> Option<ResultDispatchSyntax> {
    let node = unwrap_result_dispatch_expression(node)?;
    match node.kind() {
        Kind::Block => parse_result_dispatch_block(Some(node)),
        Kind::ConditionalExpression => parse_result_dispatch_conditional(Some(node)),
        _ => None,
    }
}

// go: internal/typeparser/returning_dispatch.go:169 parseResultDispatchBlock
pub(crate) fn parse_result_dispatch_block(node: Option<P<Node>>) -> Option<ResultDispatchSyntax> {
    let node = node?;
    if node.kind() != Kind::Block {
        return None;
    }
    let statements = node.as_block().statements().nodes();
    if statements.is_empty() {
        return None;
    }
    if statements.len() == 1 {
        match statements[0].kind() {
            Kind::SwitchStatement => return parse_result_dispatch_switch(Some(statements[0])),
            Kind::IfStatement => return parse_result_dispatch_if_else(Some(statements[0])),
            Kind::ReturnStatement => {
                let statement = statements[0].as_return_statement();
                return parse_result_dispatch_conditional(Some(statement.expression()?));
            }
            _ => {}
        }
    }
    parse_result_dispatch_sequential_ifs(statements)
}

// go: internal/typeparser/returning_dispatch.go:195 parseResultDispatchSwitch
pub(crate) fn parse_result_dispatch_switch(node: Option<P<Node>>) -> Option<ResultDispatchSyntax> {
    let node = node?;
    if node.kind() != Kind::SwitchStatement {
        return None;
    }
    let statement = node.as_switch_statement();
    if statement.case_block().kind() != Kind::CaseBlock {
        return None;
    }
    let clauses = statement.case_block().as_case_block().clauses().nodes();

    let mut dispatch = ResultDispatchSyntax::default();
    for (index, &clause_node) in clauses.iter().enumerate() {
        if clause_node.kind() != Kind::CaseClause && clause_node.kind() != Kind::DefaultClause {
            return None;
        }
        let clause = clause_node.as_case_or_default_clause();
        let result = single_result_dispatch_return(clause.statements().nodes())?;

        if clause_node.kind() == Kind::DefaultClause {
            if index != clauses.len() - 1 || dispatch.fallback.is_some() {
                return None;
            }
            dispatch.fallback = Some(result);
            continue;
        }
        let clause_expression = clause.expression()?;
        dispatch.branches.push(ResultDispatchBranch {
            condition: new_dispatch_condition(
                DispatchConditionKind::SwitchCase,
                clause_node,
                statement.expression(),
                Some(clause_expression),
            ),
            result,
        });
    }
    if dispatch.branches.is_empty() {
        return None;
    }
    Some(dispatch)
}

// go: internal/typeparser/returning_dispatch.go:243 parseResultDispatchConditional
pub(crate) fn parse_result_dispatch_conditional(
    node: Option<P<Node>>,
) -> Option<ResultDispatchSyntax> {
    let node = unwrap_result_dispatch_expression(node)?;
    if node.kind() != Kind::ConditionalExpression {
        return None;
    }

    let mut dispatch = ResultDispatchSyntax::default();
    let mut current = Some(node);
    while let Some(c) = current
        && c.kind() == Kind::ConditionalExpression
    {
        let conditional = c.as_conditional_expression();
        let when_true = unwrap_result_dispatch_expression(Some(conditional.when_true()))?;
        if when_true.kind() == Kind::ConditionalExpression {
            return None;
        }
        dispatch.branches.push(ResultDispatchBranch {
            condition: new_dispatch_condition(
                DispatchConditionKind::Predicate,
                conditional.condition(),
                conditional.condition(),
                None,
            ),
            result: conditional.when_true(),
        });

        let when_false = unwrap_result_dispatch_expression(Some(conditional.when_false()));
        if let Some(when_false) = when_false
            && when_false.kind() == Kind::ConditionalExpression
        {
            current = Some(when_false);
            continue;
        }
        dispatch.fallback = Some(conditional.when_false());
        current = None;
    }
    if dispatch.branches.is_empty() || dispatch.fallback.is_none() {
        return None;
    }
    Some(dispatch)
}

// go: internal/typeparser/returning_dispatch.go:279 parseResultDispatchIfElse
pub(crate) fn parse_result_dispatch_if_else(node: Option<P<Node>>) -> Option<ResultDispatchSyntax> {
    let node = node?;
    if node.kind() != Kind::IfStatement {
        return None;
    }
    let mut dispatch = ResultDispatchSyntax::default();
    let mut current = Some(node);
    while let Some(c) = current
        && c.kind() == Kind::IfStatement
    {
        let statement = c.as_if_statement();
        let result = single_result_dispatch_embedded_return(Some(statement.then_statement()))?;
        dispatch.branches.push(ResultDispatchBranch {
            condition: new_dispatch_condition(
                DispatchConditionKind::Predicate,
                statement.expression(),
                statement.expression(),
                None,
            ),
            result,
        });

        let Some(else_statement) = statement.else_statement() else {
            current = None;
            continue;
        };
        if else_statement.kind() == Kind::IfStatement {
            current = Some(else_statement);
            continue;
        }
        dispatch.fallback = single_result_dispatch_embedded_return(Some(else_statement));
        dispatch.fallback?;
        current = None;
    }
    if dispatch.branches.is_empty() {
        return None;
    }
    Some(dispatch)
}

// go: internal/typeparser/returning_dispatch.go:319 parseResultDispatchSequentialIfs
pub(crate) fn parse_result_dispatch_sequential_ifs(
    statements: &[P<Node>],
) -> Option<ResultDispatchSyntax> {
    if statements.is_empty() {
        return None;
    }
    let mut dispatch = ResultDispatchSyntax::default();
    let mut branch_statements = statements;
    let last = statements[statements.len() - 1];
    if last.kind() == Kind::ReturnStatement {
        let returned = last.as_return_statement();
        dispatch.fallback = Some(returned.expression()?);
        branch_statements = &statements[..statements.len() - 1];
    }
    if branch_statements.is_empty() {
        return None;
    }

    for &node in branch_statements {
        if node.kind() != Kind::IfStatement {
            return None;
        }
        let statement = node.as_if_statement();
        if statement.else_statement().is_some() {
            return None;
        }
        let result = single_result_dispatch_embedded_return(Some(statement.then_statement()))?;
        dispatch.branches.push(ResultDispatchBranch {
            condition: new_dispatch_condition(
                DispatchConditionKind::Predicate,
                statement.expression(),
                statement.expression(),
                None,
            ),
            result,
        });
    }
    Some(dispatch)
}

// go: internal/typeparser/returning_dispatch.go:358 newDispatchCondition
pub(crate) fn new_dispatch_condition(
    kind: DispatchConditionKind,
    source: P<Node>,
    subject: P<Node>,
    value: Option<P<Node>>,
) -> DispatchCondition {
    DispatchCondition { kind, source, subject, value, tag_subject: None, tag_value: None }
}

impl TypeParser<'_> {
    // go: internal/typeparser/returning_dispatch.go:367 TypeParser.dispatchConditionTagNodes
    pub(crate) fn dispatch_condition_tag_nodes(
        &mut self,
        condition: DispatchCondition,
    ) -> (Option<P<Node>>, Option<P<Node>>) {
        match condition.kind {
            DispatchConditionKind::Predicate => self.parse_tag_match(condition.subject),
            DispatchConditionKind::SwitchCase => {
                if let Some(subject) = dispatch_tag_subject(Some(condition.subject)) {
                    return (Some(subject), condition.value);
                }
                (None, None)
            }
        }
    }

    // go: internal/typeparser/returning_dispatch.go:383 TypeParser.ParseTagMatch
    // ParseTagMatch decodes a positive `_tag` equality or an Effect
    // Predicate.isTagged call, returning the original subject and tag expressions.
    pub fn parse_tag_match(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> (Option<P<Node>>, Option<P<Node>>) {
        let Some(predicate) = unwrap_result_dispatch_expression(node.into()) else {
            return (None, None);
        };
        if predicate.kind() == Kind::BinaryExpression {
            return parse_tag_equality_match(Some(predicate));
        }
        let Some(call) = result_dispatch_call(Some(predicate)) else {
            return (None, None);
        };
        let call_expression = call.as_call_expression();
        let arguments = call_expression.arguments().nodes();
        let callee = unwrap_result_dispatch_expression(Some(call_expression.expression()));
        if arguments.len() == 2
            && let Some(callee) = callee
            && self.is_node_reference_to_effect_predicate_module_api(callee, "isTagged")
        {
            return (
                unwrap_result_dispatch_expression(Some(arguments[0])),
                unwrap_result_dispatch_expression(Some(arguments[1])),
            );
        }
        if arguments.len() == 1
            && let Some(inner) = result_dispatch_call(callee)
        {
            let inner_expression = inner.as_call_expression();
            let inner_arguments = inner_expression.arguments().nodes();
            if inner_arguments.len() == 1
                && let Some(inner_callee) =
                    unwrap_result_dispatch_expression(Some(inner_expression.expression()))
                && self.is_node_reference_to_effect_predicate_module_api(inner_callee, "isTagged")
            {
                return (
                    unwrap_result_dispatch_expression(Some(arguments[0])),
                    unwrap_result_dispatch_expression(Some(inner_arguments[0])),
                );
            }
        }
        (None, None)
    }
}

// go: internal/typeparser/returning_dispatch.go:409 resultDispatchCall
// Rust: the call expression node (Go's `*ast.CallExpression`).
pub(crate) fn result_dispatch_call(node: Option<P<Node>>) -> Option<P<Node>> {
    let node = unwrap_result_dispatch_expression(node)?;
    if node.kind() != Kind::CallExpression || ets_shim::ast::is_optional_chain(node) {
        return None;
    }
    let call = node.as_call_expression();
    if unwrap_result_dispatch_expression(Some(call.expression()))
        .is_none_or(ets_shim::ast::is_optional_chain)
    {
        return None;
    }
    for &argument in call.arguments().nodes() {
        let argument = unwrap_result_dispatch_expression(Some(argument))?;
        if argument.kind() == Kind::SpreadElement {
            return None;
        }
    }
    Some(node)
}

// go: internal/typeparser/returning_dispatch.go:428 parseTagEqualityMatch
pub(crate) fn parse_tag_equality_match(
    node: Option<P<Node>>,
) -> (Option<P<Node>>, Option<P<Node>>) {
    let Some(predicate) = unwrap_result_dispatch_expression(node) else {
        return (None, None);
    };
    if predicate.kind() != Kind::BinaryExpression {
        return (None, None);
    }
    let binary = predicate.as_binary_expression();
    let operator = binary.operator_token().kind();
    if operator != Kind::EqualsEqualsToken && operator != Kind::EqualsEqualsEqualsToken {
        return (None, None);
    }

    if let Some(subject) = dispatch_tag_subject(Some(binary.left())) {
        return (Some(subject), unwrap_result_dispatch_expression(Some(binary.right())));
    }
    if let Some(subject) = dispatch_tag_subject(Some(binary.right())) {
        return (Some(subject), unwrap_result_dispatch_expression(Some(binary.left())));
    }
    (None, None)
}

// go: internal/typeparser/returning_dispatch.go:448 dispatchTagSubject
// Rust: Go's `(subject, ok)`; `ok` with a nil subject (an unwrapped expression that is nil) cannot happen, as a
// property access always has an expression, so the pair is an `Option`.
pub(crate) fn dispatch_tag_subject(node: Option<P<Node>>) -> Option<P<Node>> {
    let node = unwrap_result_dispatch_expression(node)?;
    if node.kind() != Kind::PropertyAccessExpression {
        return None;
    }
    let access = node.as_property_access_expression();
    if access.name().text() != "_tag" {
        return None;
    }
    unwrap_result_dispatch_expression(Some(access.expression()))
}

// go: internal/typeparser/returning_dispatch.go:460 singleResultDispatchEmbeddedReturn
pub(crate) fn single_result_dispatch_embedded_return(
    statement: Option<P<Node>>,
) -> Option<P<Node>> {
    let statement = statement?;
    if statement.kind() == Kind::ReturnStatement {
        return statement.as_return_statement().expression();
    }
    if statement.kind() != Kind::Block {
        return None;
    }
    single_result_dispatch_return(statement.as_block().statements().nodes())
}

// go: internal/typeparser/returning_dispatch.go:481 singleResultDispatchReturn
pub(crate) fn single_result_dispatch_return(statements: &[P<Node>]) -> Option<P<Node>> {
    if statements.len() != 1 || statements[0].kind() != Kind::ReturnStatement {
        return None;
    }
    statements[0].as_return_statement().expression()
}

// go: internal/typeparser/returning_dispatch.go:492 unwrapResultDispatchExpression
pub(crate) fn unwrap_result_dispatch_expression(node: Option<P<Node>>) -> Option<P<Node>> {
    let mut node = node;
    while let Some(n) = node {
        match n.kind() {
            Kind::ParenthesizedExpression
            | Kind::SatisfiesExpression
            | Kind::AsExpression
            | Kind::NonNullExpression
            | Kind::TypeAssertionExpression => node = n.expression(),
            _ => return Some(n),
        }
    }
    None
}
