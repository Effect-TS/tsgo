package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// ReturningDispatchBranch is one source-ordered branch in a decoded
// result-producing function body. When Discriminant is nil, Test is a boolean
// predicate from an if or conditional expression. When Discriminant is set,
// Test is the corresponding switch case expression.
type ReturningDispatchBranch struct {
	Test         *ast.Node
	TestNode     *ast.Node
	Discriminant *ast.Node
	Result       *ast.Node
}

// ParsedReturningDispatch is a decoded arrow function or function expression
// whose body consists entirely of result-producing branches.
type ParsedReturningDispatch struct {
	Node     *ast.Node
	Params   []*ast.Node
	Body     *ast.Node
	Branches []ReturningDispatchBranch
	Fallback *ast.Node
}

type returningDispatchSyntax struct {
	branches []ReturningDispatchBranch
	fallback *ast.Node
}

// ParseReturningDispatch decodes an arrow function or function expression into
// source-ordered result branches and an optional fallback. It recognizes
// conditional expressions, returned conditional expressions, if/else-if,
// sequential returning if statements, and returning switch cases.
func ParseReturningDispatch(node *ast.Node) *ParsedReturningDispatch {
	node = unwrapReturningDispatchExpression(node)
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

	syntax := parseReturningDispatchBody(body)
	if syntax == nil || len(syntax.branches) == 0 {
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
		Branches: syntax.branches,
		Fallback: syntax.fallback,
	}
}

func parseReturningDispatchBody(node *ast.Node) *returningDispatchSyntax {
	node = unwrapReturningDispatchExpression(node)
	if node == nil {
		return nil
	}
	switch node.Kind {
	case ast.KindBlock:
		return parseReturningDispatchBlock(node)
	case ast.KindConditionalExpression:
		return parseReturningDispatchConditional(node)
	default:
		return nil
	}
}

func parseReturningDispatchBlock(node *ast.Node) *returningDispatchSyntax {
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
			return parseReturningDispatchSwitch(statements[0])
		case ast.KindIfStatement:
			return parseReturningDispatchIfElse(statements[0])
		case ast.KindReturnStatement:
			statement := statements[0].AsReturnStatement()
			if statement == nil || statement.Expression == nil {
				return nil
			}
			return parseReturningDispatchConditional(statement.Expression)
		}
	}
	return parseReturningDispatchSequentialIfs(statements)
}

func parseReturningDispatchSwitch(node *ast.Node) *returningDispatchSyntax {
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

	dispatch := &returningDispatchSyntax{}
	for index, clauseNode := range caseBlock.Clauses.Nodes {
		if clauseNode == nil || (clauseNode.Kind != ast.KindCaseClause && clauseNode.Kind != ast.KindDefaultClause) {
			return nil
		}
		clause := clauseNode.AsCaseOrDefaultClause()
		if clause == nil || clause.Statements == nil {
			return nil
		}
		result := singleReturningDispatchReturn(clause.Statements.Nodes)
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
		dispatch.branches = append(dispatch.branches, ReturningDispatchBranch{
			Test:         clause.Expression,
			TestNode:     clauseNode,
			Discriminant: statement.Expression,
			Result:       result,
		})
	}
	if len(dispatch.branches) == 0 {
		return nil
	}
	return dispatch
}

func parseReturningDispatchConditional(node *ast.Node) *returningDispatchSyntax {
	node = unwrapReturningDispatchExpression(node)
	if node == nil || node.Kind != ast.KindConditionalExpression {
		return nil
	}

	dispatch := &returningDispatchSyntax{}
	current := node
	for current != nil && current.Kind == ast.KindConditionalExpression {
		conditional := current.AsConditionalExpression()
		if conditional == nil || conditional.Condition == nil || conditional.WhenTrue == nil || conditional.WhenFalse == nil {
			return nil
		}
		whenTrue := unwrapReturningDispatchExpression(conditional.WhenTrue)
		if whenTrue == nil || whenTrue.Kind == ast.KindConditionalExpression {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ReturningDispatchBranch{
			Test:     conditional.Condition,
			TestNode: conditional.Condition,
			Result:   conditional.WhenTrue,
		})

		whenFalse := unwrapReturningDispatchExpression(conditional.WhenFalse)
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

func parseReturningDispatchIfElse(node *ast.Node) *returningDispatchSyntax {
	if node == nil || node.Kind != ast.KindIfStatement {
		return nil
	}
	dispatch := &returningDispatchSyntax{}
	current := node
	for current != nil && current.Kind == ast.KindIfStatement {
		statement := current.AsIfStatement()
		if statement == nil || statement.Expression == nil || statement.ThenStatement == nil {
			return nil
		}
		result := singleReturningDispatchEmbeddedReturn(statement.ThenStatement)
		if result == nil {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ReturningDispatchBranch{
			Test:     statement.Expression,
			TestNode: statement.Expression,
			Result:   result,
		})

		if statement.ElseStatement == nil {
			current = nil
			continue
		}
		if statement.ElseStatement.Kind == ast.KindIfStatement {
			current = statement.ElseStatement
			continue
		}
		dispatch.fallback = singleReturningDispatchEmbeddedReturn(statement.ElseStatement)
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

func parseReturningDispatchSequentialIfs(statements []*ast.Node) *returningDispatchSyntax {
	if len(statements) == 0 {
		return nil
	}
	dispatch := &returningDispatchSyntax{}
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
		result := singleReturningDispatchEmbeddedReturn(statement.ThenStatement)
		if result == nil {
			return nil
		}
		dispatch.branches = append(dispatch.branches, ReturningDispatchBranch{
			Test:     statement.Expression,
			TestNode: statement.Expression,
			Result:   result,
		})
	}
	return dispatch
}

func singleReturningDispatchEmbeddedReturn(statement *ast.Node) *ast.Node {
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
	return singleReturningDispatchReturn(block.Statements.Nodes)
}

func singleReturningDispatchReturn(statements []*ast.Node) *ast.Node {
	if len(statements) != 1 || statements[0] == nil || statements[0].Kind != ast.KindReturnStatement {
		return nil
	}
	returned := statements[0].AsReturnStatement()
	if returned == nil {
		return nil
	}
	return returned.Expression
}

func unwrapReturningDispatchExpression(node *ast.Node) *ast.Node {
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
