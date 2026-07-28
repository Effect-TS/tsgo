package typeparser

import "github.com/microsoft/typescript-go/shim/ast"

// TaggedDispatchBranch is one string-literal branch in a decoded _tag dispatch.
type TaggedDispatchBranch struct {
	Tag          string
	TagNode      *ast.Node
	TestNode     *ast.Node
	Discriminant *ast.Node
	Result       *ast.Node
}

// TaggedDispatch contains source-ordered tagged branches and an optional fallback.
type TaggedDispatch struct {
	Branches []TaggedDispatchBranch
	Fallback *ast.Node
}

// TaggedDispatchDiscriminantPredicate validates each candidate _tag access.
// Callers own symbol and type semantics; the decoder only validates syntax.
type TaggedDispatchDiscriminantPredicate func(discriminant *ast.Node) bool

// ParseTaggedDispatch decodes conservative switch, conditional-expression, and
// if/else-if dispatch over string-literal _tag comparisons.
func (tp *TypeParser) ParseTaggedDispatch(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) *TaggedDispatch {
	if tp == nil || node == nil || accept == nil {
		return nil
	}

	node = unwrapTaggedDispatchExpression(node)
	if node == nil {
		return nil
	}
	switch node.Kind {
	case ast.KindBlock:
		return parseTaggedDispatchBlock(node, accept)
	case ast.KindSwitchStatement:
		return parseTaggedDispatchSwitch(node, accept)
	case ast.KindIfStatement:
		return parseTaggedDispatchIfElse(node, accept)
	case ast.KindConditionalExpression:
		return parseTaggedDispatchConditional(node, accept)
	case ast.KindReturnStatement:
		statement := node.AsReturnStatement()
		if statement == nil || statement.Expression == nil {
			return nil
		}
		return parseTaggedDispatchConditional(statement.Expression, accept)
	default:
		return nil
	}
}

func parseTaggedDispatchBlock(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) *TaggedDispatch {
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
			return parseTaggedDispatchSwitch(statements[0], accept)
		case ast.KindIfStatement:
			return parseTaggedDispatchIfElse(statements[0], accept)
		case ast.KindReturnStatement:
			statement := statements[0].AsReturnStatement()
			if statement == nil || statement.Expression == nil {
				return nil
			}
			return parseTaggedDispatchConditional(statement.Expression, accept)
		}
	}
	return parseTaggedDispatchSequentialIfs(statements, accept)
}

func parseTaggedDispatchSwitch(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) *TaggedDispatch {
	if node == nil || node.Kind != ast.KindSwitchStatement {
		return nil
	}
	statement := node.AsSwitchStatement()
	if statement == nil || statement.Expression == nil || statement.CaseBlock == nil || statement.CaseBlock.Kind != ast.KindCaseBlock {
		return nil
	}
	discriminant, ok := acceptedTaggedDiscriminant(statement.Expression, accept)
	if !ok {
		return nil
	}
	caseBlock := statement.CaseBlock.AsCaseBlock()
	if caseBlock == nil || caseBlock.Clauses == nil {
		return nil
	}

	dispatch := &TaggedDispatch{}
	seen := make(map[string]struct{})
	for index, clauseNode := range caseBlock.Clauses.Nodes {
		if clauseNode == nil || (clauseNode.Kind != ast.KindCaseClause && clauseNode.Kind != ast.KindDefaultClause) {
			return nil
		}
		clause := clauseNode.AsCaseOrDefaultClause()
		if clause == nil || clause.Statements == nil {
			return nil
		}
		result := singleTaggedDispatchReturn(clause.Statements.Nodes)
		if result == nil {
			return nil
		}

		if clauseNode.Kind == ast.KindDefaultClause {
			if index != len(caseBlock.Clauses.Nodes)-1 || dispatch.Fallback != nil {
				return nil
			}
			dispatch.Fallback = result
			continue
		}

		tagNode := unwrapTaggedDispatchExpression(clause.Expression)
		if tagNode == nil || !ast.IsStringLiteral(tagNode) {
			return nil
		}
		branch := TaggedDispatchBranch{
			Tag:          tagNode.AsStringLiteral().Text,
			TagNode:      tagNode,
			TestNode:     clauseNode,
			Discriminant: discriminant,
			Result:       result,
		}
		if !appendTaggedDispatchBranch(dispatch, seen, branch) {
			return nil
		}
	}
	if len(dispatch.Branches) == 0 {
		return nil
	}
	return dispatch
}

func parseTaggedDispatchConditional(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) *TaggedDispatch {
	node = unwrapTaggedDispatchExpression(node)
	if node == nil || node.Kind != ast.KindConditionalExpression {
		return nil
	}

	dispatch := &TaggedDispatch{}
	seen := make(map[string]struct{})
	current := node
	for current != nil && current.Kind == ast.KindConditionalExpression {
		conditional := current.AsConditionalExpression()
		if conditional == nil || conditional.Condition == nil || conditional.WhenTrue == nil || conditional.WhenFalse == nil {
			return nil
		}
		branch, ok := parseTaggedDispatchComparison(conditional.Condition, accept)
		whenTrue := unwrapTaggedDispatchExpression(conditional.WhenTrue)
		if !ok || whenTrue == nil || whenTrue.Kind == ast.KindConditionalExpression {
			return nil
		}
		branch.Result = conditional.WhenTrue
		if !appendTaggedDispatchBranch(dispatch, seen, branch) {
			return nil
		}

		whenFalse := unwrapTaggedDispatchExpression(conditional.WhenFalse)
		if whenFalse != nil && whenFalse.Kind == ast.KindConditionalExpression {
			current = whenFalse
			continue
		}
		dispatch.Fallback = conditional.WhenFalse
		current = nil
	}
	if len(dispatch.Branches) == 0 || dispatch.Fallback == nil {
		return nil
	}
	return dispatch
}

func parseTaggedDispatchIfElse(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) *TaggedDispatch {
	if node == nil || node.Kind != ast.KindIfStatement {
		return nil
	}
	dispatch := &TaggedDispatch{}
	seen := make(map[string]struct{})
	current := node
	for current != nil && current.Kind == ast.KindIfStatement {
		statement := current.AsIfStatement()
		if statement == nil || statement.Expression == nil || statement.ThenStatement == nil {
			return nil
		}
		branch, ok := parseTaggedDispatchComparison(statement.Expression, accept)
		if !ok {
			return nil
		}
		branch.Result = singleTaggedDispatchEmbeddedReturn(statement.ThenStatement)
		if branch.Result == nil || !appendTaggedDispatchBranch(dispatch, seen, branch) {
			return nil
		}

		if statement.ElseStatement == nil {
			current = nil
			continue
		}
		if statement.ElseStatement.Kind == ast.KindIfStatement {
			current = statement.ElseStatement
			continue
		}
		dispatch.Fallback = singleTaggedDispatchEmbeddedReturn(statement.ElseStatement)
		if dispatch.Fallback == nil {
			return nil
		}
		current = nil
	}
	if len(dispatch.Branches) == 0 {
		return nil
	}
	return dispatch
}

func parseTaggedDispatchSequentialIfs(statements []*ast.Node, accept TaggedDispatchDiscriminantPredicate) *TaggedDispatch {
	if len(statements) == 0 {
		return nil
	}
	dispatch := &TaggedDispatch{}
	seen := make(map[string]struct{})
	branchStatements := statements
	last := statements[len(statements)-1]
	if last != nil && last.Kind == ast.KindReturnStatement {
		returned := last.AsReturnStatement()
		if returned == nil || returned.Expression == nil {
			return nil
		}
		dispatch.Fallback = returned.Expression
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
		branch, ok := parseTaggedDispatchComparison(statement.Expression, accept)
		if !ok {
			return nil
		}
		branch.Result = singleTaggedDispatchEmbeddedReturn(statement.ThenStatement)
		if branch.Result == nil || !appendTaggedDispatchBranch(dispatch, seen, branch) {
			return nil
		}
	}
	return dispatch
}

func parseTaggedDispatchComparison(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) (TaggedDispatchBranch, bool) {
	node = unwrapTaggedDispatchExpression(node)
	if node == nil || node.Kind != ast.KindBinaryExpression {
		return TaggedDispatchBranch{}, false
	}
	binary := node.AsBinaryExpression()
	if binary == nil || binary.Left == nil || binary.Right == nil || binary.OperatorToken == nil ||
		(binary.OperatorToken.Kind != ast.KindEqualsEqualsToken && binary.OperatorToken.Kind != ast.KindEqualsEqualsEqualsToken) {
		return TaggedDispatchBranch{}, false
	}

	left := unwrapTaggedDispatchExpression(binary.Left)
	right := unwrapTaggedDispatchExpression(binary.Right)
	var tagNode *ast.Node
	var discriminant *ast.Node
	var ok bool
	if left != nil && ast.IsStringLiteral(left) {
		tagNode = left
		discriminant, ok = acceptedTaggedDiscriminant(right, accept)
	} else if right != nil && ast.IsStringLiteral(right) {
		tagNode = right
		discriminant, ok = acceptedTaggedDiscriminant(left, accept)
	}
	if !ok || tagNode == nil {
		return TaggedDispatchBranch{}, false
	}
	return TaggedDispatchBranch{
		Tag:          tagNode.AsStringLiteral().Text,
		TagNode:      tagNode,
		TestNode:     node,
		Discriminant: discriminant,
	}, true
}

func acceptedTaggedDiscriminant(node *ast.Node, accept TaggedDispatchDiscriminantPredicate) (*ast.Node, bool) {
	node = unwrapTaggedDispatchExpression(node)
	if node == nil || node.Kind != ast.KindPropertyAccessExpression {
		return nil, false
	}
	access := node.AsPropertyAccessExpression()
	if access == nil || access.Expression == nil || access.Name() == nil || access.Name().Text() != "_tag" || !accept(node) {
		return nil, false
	}
	return node, true
}

func appendTaggedDispatchBranch(dispatch *TaggedDispatch, seen map[string]struct{}, branch TaggedDispatchBranch) bool {
	if dispatch == nil || branch.TagNode == nil || branch.TestNode == nil || branch.Discriminant == nil || branch.Result == nil {
		return false
	}
	if _, duplicate := seen[branch.Tag]; duplicate {
		return false
	}
	seen[branch.Tag] = struct{}{}
	dispatch.Branches = append(dispatch.Branches, branch)
	return true
}

func singleTaggedDispatchEmbeddedReturn(statement *ast.Node) *ast.Node {
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
	return singleTaggedDispatchReturn(block.Statements.Nodes)
}

func singleTaggedDispatchReturn(statements []*ast.Node) *ast.Node {
	if len(statements) != 1 || statements[0] == nil || statements[0].Kind != ast.KindReturnStatement {
		return nil
	}
	returned := statements[0].AsReturnStatement()
	if returned == nil {
		return nil
	}
	return returned.Expression
}

func unwrapTaggedDispatchExpression(node *ast.Node) *ast.Node {
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
