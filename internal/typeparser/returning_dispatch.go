package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// DispatchConditionKind identifies how one result-producing branch is
// selected.
type DispatchConditionKind uint8

const (
	// DispatchConditionPredicate is a boolean condition from an if or
	// conditional expression.
	DispatchConditionPredicate DispatchConditionKind = iota
	// DispatchConditionSwitchCase compares Subject, the switch
	// discriminant, with Value, the case expression.
	DispatchConditionSwitchCase
)

// DispatchCondition preserves the source nodes that select one branch.
// Predicate conditions set Subject to the condition expression and leave Value
// nil. Switch cases set Subject to the shared discriminant and Value to the
// case expression. Source is the predicate expression or case clause used for
// diagnostics. TagSubject and TagValue are populated for `_tag` equality and
// Effect Predicate.isTagged conditions.
type DispatchCondition struct {
	Kind       DispatchConditionKind
	Source     *ast.Node
	Subject    *ast.Node
	Value      *ast.Node
	TagSubject *ast.Node
	TagValue   *ast.Node
}

// ResultDispatchBranch is one source-ordered branch in a decoded
// result-producing expression or block.
type ResultDispatchBranch struct {
	Condition DispatchCondition
	Result    *ast.Node
}

// ResultDispatch is an ordered, first-match result dispatch with an
// optional fallback.
type ResultDispatch struct {
	Node     *ast.Node
	Branches []ResultDispatchBranch
	Fallback *ast.Node
}

// ParsedReturningDispatch is a decoded arrow function or function expression
// whose body consists entirely of result-producing branches.
type ParsedReturningDispatch struct {
	Node     *ast.Node
	Params   []*ast.Node
	Body     *ast.Node
	Dispatch *ResultDispatch
}

type resultDispatchSyntax struct {
	branches []ResultDispatchBranch
	fallback *ast.Node
}

// ParseResultDispatch decodes a result-producing conditional expression or
// block into source-ordered branches and an optional fallback. False-arm
// conditional chains are flattened; conditionals in a true arm are rejected.
// Tag hints include checker-backed recognition of Effect Predicate.isTagged.
func (tp *TypeParser) ParseResultDispatch(node *ast.Node) *ResultDispatch {
	if node == nil {
		return nil
	}
	syntax := parseResultDispatchBody(node)
	if syntax == nil || len(syntax.branches) == 0 {
		return nil
	}
	for i := range syntax.branches {
		condition := &syntax.branches[i].Condition
		condition.TagSubject, condition.TagValue = tp.dispatchConditionTagNodes(*condition)
	}
	return &ResultDispatch{
		Node:     node,
		Branches: syntax.branches,
		Fallback: syntax.fallback,
	}
}

// ParseReturningDispatch decodes an arrow function or function expression into
// source-ordered result branches and an optional fallback. It recognizes
// conditional expressions, returned conditional expressions, if/else-if,
// sequential returning if statements, and returning switch cases.
func (tp *TypeParser) ParseReturningDispatch(node *ast.Node) *ParsedReturningDispatch {
	node = unwrapResultDispatchExpression(node)
	if node == nil || (node.Kind != ast.KindArrowFunction && node.Kind != ast.KindFunctionExpression) {
		return nil
	}
	typeParameters := GetFunctionLikeTypeParameters(node)
	if typeParameters != nil && len(typeParameters.Nodes) > 0 {
		return nil
	}
	body := GetFunctionLikeBody(node)
	if body == nil {
		return nil
	}

	dispatch := tp.ParseResultDispatch(body)
	if dispatch == nil {
		return nil
	}
	parameters := GetFunctionLikeParameters(node)
	var params []*ast.Node
	if parameters != nil {
		params = parameters.Nodes
	}
	return &ParsedReturningDispatch{
		Node:     node,
		Params:   params,
		Body:     body,
		Dispatch: dispatch,
	}
}

func parseResultDispatchBody(node *ast.Node) *resultDispatchSyntax {
	node = unwrapResultDispatchExpression(node)
	if node == nil {
		return nil
	}
	switch node.Kind {
	case ast.KindBlock:
		return parseResultDispatchBlock(node)
	case ast.KindConditionalExpression:
		return parseResultDispatchConditional(node)
	default:
		return nil
	}
}

func parseResultDispatchBlock(node *ast.Node) *resultDispatchSyntax {
	if node == nil || node.Kind != ast.KindBlock {
		return nil
	}
	block := node.AsBlock()
	if block == nil || block.Statements == nil || len(block.Statements.Nodes) == 0 {
		return nil
	}
	statements := block.Statements.Nodes
	if len(statements) == 1 && statements[0] != nil {
		switch statements[0].Kind {
		case ast.KindSwitchStatement:
			return parseResultDispatchSwitch(statements[0])
		case ast.KindIfStatement:
			return parseResultDispatchIfElse(statements[0])
		case ast.KindReturnStatement:
			statement := statements[0].AsReturnStatement()
			if statement == nil || statement.Expression == nil {
				return nil
			}
			return parseResultDispatchConditional(statement.Expression)
		}
	}
	return parseResultDispatchSequentialIfs(statements)
}

func parseResultDispatchSwitch(node *ast.Node) *resultDispatchSyntax {
	if node == nil || node.Kind != ast.KindSwitchStatement {
		return nil
	}
	statement := node.AsSwitchStatement()
	if statement == nil || statement.Expression == nil || statement.CaseBlock == nil || statement.CaseBlock.Kind != ast.KindCaseBlock {
		return nil
	}
	caseBlock := statement.CaseBlock.AsCaseBlock()
	if caseBlock == nil || caseBlock.Clauses == nil {
		return nil
	}

	dispatch := &resultDispatchSyntax{}
	for index, clauseNode := range caseBlock.Clauses.Nodes {
		if clauseNode == nil || (clauseNode.Kind != ast.KindCaseClause && clauseNode.Kind != ast.KindDefaultClause) {
			return nil
		}
		clause := clauseNode.AsCaseOrDefaultClause()
		if clause == nil || clause.Statements == nil {
			return nil
		}
		result := singleResultDispatchReturn(clause.Statements.Nodes)
		if result == nil {
			return nil
		}

		if clauseNode.Kind == ast.KindDefaultClause {
			if index != len(caseBlock.Clauses.Nodes)-1 || dispatch.fallback != nil {
				return nil
			}
			dispatch.fallback = result
			continue
		}
		if clause.Expression == nil {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ResultDispatchBranch{
			Condition: newDispatchCondition(DispatchConditionSwitchCase, clauseNode, statement.Expression, clause.Expression),
			Result:    result,
		})
	}
	if len(dispatch.branches) == 0 {
		return nil
	}
	return dispatch
}

func parseResultDispatchConditional(node *ast.Node) *resultDispatchSyntax {
	node = unwrapResultDispatchExpression(node)
	if node == nil || node.Kind != ast.KindConditionalExpression {
		return nil
	}

	dispatch := &resultDispatchSyntax{}
	current := node
	for current != nil && current.Kind == ast.KindConditionalExpression {
		conditional := current.AsConditionalExpression()
		if conditional == nil || conditional.Condition == nil || conditional.WhenTrue == nil || conditional.WhenFalse == nil {
			return nil
		}
		whenTrue := unwrapResultDispatchExpression(conditional.WhenTrue)
		if whenTrue == nil || whenTrue.Kind == ast.KindConditionalExpression {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ResultDispatchBranch{
			Condition: newDispatchCondition(DispatchConditionPredicate, conditional.Condition, conditional.Condition, nil),
			Result:    conditional.WhenTrue,
		})

		whenFalse := unwrapResultDispatchExpression(conditional.WhenFalse)
		if whenFalse != nil && whenFalse.Kind == ast.KindConditionalExpression {
			current = whenFalse
			continue
		}
		dispatch.fallback = conditional.WhenFalse
		current = nil
	}
	if len(dispatch.branches) == 0 || dispatch.fallback == nil {
		return nil
	}
	return dispatch
}

func parseResultDispatchIfElse(node *ast.Node) *resultDispatchSyntax {
	if node == nil || node.Kind != ast.KindIfStatement {
		return nil
	}
	dispatch := &resultDispatchSyntax{}
	current := node
	for current != nil && current.Kind == ast.KindIfStatement {
		statement := current.AsIfStatement()
		if statement == nil || statement.Expression == nil || statement.ThenStatement == nil {
			return nil
		}
		result := singleResultDispatchEmbeddedReturn(statement.ThenStatement)
		if result == nil {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ResultDispatchBranch{
			Condition: newDispatchCondition(DispatchConditionPredicate, statement.Expression, statement.Expression, nil),
			Result:    result,
		})

		if statement.ElseStatement == nil {
			current = nil
			continue
		}
		if statement.ElseStatement.Kind == ast.KindIfStatement {
			current = statement.ElseStatement
			continue
		}
		dispatch.fallback = singleResultDispatchEmbeddedReturn(statement.ElseStatement)
		if dispatch.fallback == nil {
			return nil
		}
		current = nil
	}
	if len(dispatch.branches) == 0 {
		return nil
	}
	return dispatch
}

func parseResultDispatchSequentialIfs(statements []*ast.Node) *resultDispatchSyntax {
	if len(statements) == 0 {
		return nil
	}
	dispatch := &resultDispatchSyntax{}
	branchStatements := statements
	last := statements[len(statements)-1]
	if last != nil && last.Kind == ast.KindReturnStatement {
		returned := last.AsReturnStatement()
		if returned == nil || returned.Expression == nil {
			return nil
		}
		dispatch.fallback = returned.Expression
		branchStatements = statements[:len(statements)-1]
	}
	if len(branchStatements) == 0 {
		return nil
	}

	for _, node := range branchStatements {
		if node == nil || node.Kind != ast.KindIfStatement {
			return nil
		}
		statement := node.AsIfStatement()
		if statement == nil || statement.Expression == nil || statement.ThenStatement == nil || statement.ElseStatement != nil {
			return nil
		}
		result := singleResultDispatchEmbeddedReturn(statement.ThenStatement)
		if result == nil {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ResultDispatchBranch{
			Condition: newDispatchCondition(DispatchConditionPredicate, statement.Expression, statement.Expression, nil),
			Result:    result,
		})
	}
	return dispatch
}

func newDispatchCondition(kind DispatchConditionKind, source *ast.Node, subject *ast.Node, value *ast.Node) DispatchCondition {
	return DispatchCondition{
		Kind:    kind,
		Source:  source,
		Subject: subject,
		Value:   value,
	}
}

func (tp *TypeParser) dispatchConditionTagNodes(condition DispatchCondition) (tagSubject *ast.Node, tagValue *ast.Node) {
	switch condition.Kind {
	case DispatchConditionPredicate:
		return tp.ParseTagMatch(condition.Subject)
	case DispatchConditionSwitchCase:
		if subject, ok := dispatchTagSubject(condition.Subject); ok {
			return subject, condition.Value
		}
		return nil, nil
	default:
		return nil, nil
	}
}

// ParseTagMatch decodes a positive `_tag` equality or an Effect
// Predicate.isTagged call, returning the original subject and tag expressions.
func (tp *TypeParser) ParseTagMatch(node *ast.Node) (tagSubject *ast.Node, tagValue *ast.Node) {
	predicate := unwrapResultDispatchExpression(node)
	if predicate == nil {
		return nil, nil
	}
	if predicate.Kind == ast.KindBinaryExpression {
		return parseTagEqualityMatch(predicate)
	}
	call := resultDispatchCall(predicate)
	if call == nil {
		return nil, nil
	}
	callee := unwrapResultDispatchExpression(call.Expression)
	if len(call.Arguments.Nodes) == 2 && tp.IsNodeReferenceToEffectPredicateModuleApi(callee, "isTagged") {
		return unwrapResultDispatchExpression(call.Arguments.Nodes[0]), unwrapResultDispatchExpression(call.Arguments.Nodes[1])
	}
	if len(call.Arguments.Nodes) == 1 {
		inner := resultDispatchCall(callee)
		if inner != nil && len(inner.Arguments.Nodes) == 1 &&
			tp.IsNodeReferenceToEffectPredicateModuleApi(unwrapResultDispatchExpression(inner.Expression), "isTagged") {
			return unwrapResultDispatchExpression(call.Arguments.Nodes[0]), unwrapResultDispatchExpression(inner.Arguments.Nodes[0])
		}
	}
	return nil, nil
}

func resultDispatchCall(node *ast.Node) *ast.CallExpression {
	node = unwrapResultDispatchExpression(node)
	if node == nil || node.Kind != ast.KindCallExpression || ast.IsOptionalChain(node) {
		return nil
	}
	call := node.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil ||
		ast.IsOptionalChain(unwrapResultDispatchExpression(call.Expression)) {
		return nil
	}
	for _, argument := range call.Arguments.Nodes {
		argument = unwrapResultDispatchExpression(argument)
		if argument == nil || argument.Kind == ast.KindSpreadElement {
			return nil
		}
	}
	return call
}

func parseTagEqualityMatch(node *ast.Node) (tagSubject *ast.Node, tagValue *ast.Node) {
	predicate := unwrapResultDispatchExpression(node)
	if predicate == nil || predicate.Kind != ast.KindBinaryExpression {
		return nil, nil
	}
	binary := predicate.AsBinaryExpression()
	if binary == nil || binary.Left == nil || binary.Right == nil || binary.OperatorToken == nil ||
		(binary.OperatorToken.Kind != ast.KindEqualsEqualsToken && binary.OperatorToken.Kind != ast.KindEqualsEqualsEqualsToken) {
		return nil, nil
	}

	if subject, ok := dispatchTagSubject(binary.Left); ok {
		return subject, unwrapResultDispatchExpression(binary.Right)
	}
	if subject, ok := dispatchTagSubject(binary.Right); ok {
		return subject, unwrapResultDispatchExpression(binary.Left)
	}
	return nil, nil
}

func dispatchTagSubject(node *ast.Node) (*ast.Node, bool) {
	node = unwrapResultDispatchExpression(node)
	if node == nil || node.Kind != ast.KindPropertyAccessExpression {
		return nil, false
	}
	access := node.AsPropertyAccessExpression()
	if access == nil || access.Expression == nil || access.Name() == nil || access.Name().Text() != "_tag" {
		return nil, false
	}
	return unwrapResultDispatchExpression(access.Expression), true
}

func singleResultDispatchEmbeddedReturn(statement *ast.Node) *ast.Node {
	if statement == nil {
		return nil
	}
	if statement.Kind == ast.KindReturnStatement {
		returned := statement.AsReturnStatement()
		if returned != nil {
			return returned.Expression
		}
		return nil
	}
	if statement.Kind != ast.KindBlock {
		return nil
	}
	block := statement.AsBlock()
	if block == nil || block.Statements == nil {
		return nil
	}
	return singleResultDispatchReturn(block.Statements.Nodes)
}

func singleResultDispatchReturn(statements []*ast.Node) *ast.Node {
	if len(statements) != 1 || statements[0] == nil || statements[0].Kind != ast.KindReturnStatement {
		return nil
	}
	returned := statements[0].AsReturnStatement()
	if returned == nil {
		return nil
	}
	return returned.Expression
}

func unwrapResultDispatchExpression(node *ast.Node) *ast.Node {
	for node != nil {
		switch node.Kind {
		case ast.KindParenthesizedExpression, ast.KindSatisfiesExpression, ast.KindAsExpression, ast.KindNonNullExpression, ast.KindTypeAssertionExpression:
			node = node.Expression()
		default:
			return node
		}
	}
	return nil
}
