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

// ParseTaggedDispatch decodes conservative switch, conditional-expression, and
// if/else-if dispatch over string-literal _tag comparisons rooted at rootSymbol.
// Consumers that require one property chain must post-validate discriminant
// chain equality; branches are only guaranteed to share rootSymbol.
func (tp *TypeParser) ParseTaggedDispatch(node *ast.Node, rootSymbol *ast.Symbol) *TaggedDispatch {
	if tp == nil || tp.checker == nil || node == nil || rootSymbol == nil {
		return nil
	}
	dispatch := parseTaggedDispatchSyntax(node)
	if dispatch == nil {
		return nil
	}
	for _, branch := range dispatch.Branches {
		if !tp.isTaggedDispatchDiscriminant(branch.Discriminant, rootSymbol) {
			return nil
		}
	}
	return dispatch
}

func parseTaggedDispatchSyntax(node *ast.Node) *TaggedDispatch {
	node = unwrapTaggedDispatchExpression(node)
	if node == nil {
		return nil
	}
	switch node.Kind {
	case ast.KindBlock:
		return parseTaggedDispatchBlock(node)
	case ast.KindSwitchStatement:
		return parseTaggedDispatchSwitch(node)
	case ast.KindIfStatement:
		return parseTaggedDispatchIfElse(node)
	case ast.KindConditionalExpression:
		return parseTaggedDispatchConditional(node)
	case ast.KindReturnStatement:
		statement := node.AsReturnStatement()
		if statement == nil || statement.Expression == nil {
			return nil
		}
		return parseTaggedDispatchConditional(statement.Expression)
	default:
		return nil
	}
}

func parseTaggedDispatchBlock(node *ast.Node) *TaggedDispatch {
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
			return parseTaggedDispatchSwitch(statements[0])
		case ast.KindIfStatement:
			return parseTaggedDispatchIfElse(statements[0])
		case ast.KindReturnStatement:
			statement := statements[0].AsReturnStatement()
			if statement == nil || statement.Expression == nil {
				return nil
			}
			return parseTaggedDispatchConditional(statement.Expression)
		}
	}
	return parseTaggedDispatchSequentialIfs(statements)
}

func parseTaggedDispatchSwitch(node *ast.Node) *TaggedDispatch {
	if node == nil || node.Kind != ast.KindSwitchStatement {
		return nil
	}
	statement := node.AsSwitchStatement()
	if statement == nil || statement.Expression == nil || statement.CaseBlock == nil || statement.CaseBlock.Kind != ast.KindCaseBlock {
		return nil
	}
	discriminant, ok := taggedDispatchDiscriminant(statement.Expression)
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

func parseTaggedDispatchConditional(node *ast.Node) *TaggedDispatch {
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
		branch, ok := parseTaggedDispatchComparison(conditional.Condition)
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

func parseTaggedDispatchIfElse(node *ast.Node) *TaggedDispatch {
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
		branch, ok := parseTaggedDispatchComparison(statement.Expression)
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

func parseTaggedDispatchSequentialIfs(statements []*ast.Node) *TaggedDispatch {
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
		branch, ok := parseTaggedDispatchComparison(statement.Expression)
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

func parseTaggedDispatchComparison(node *ast.Node) (TaggedDispatchBranch, bool) {
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
		discriminant, ok = taggedDispatchDiscriminant(right)
	} else if right != nil && ast.IsStringLiteral(right) {
		tagNode = right
		discriminant, ok = taggedDispatchDiscriminant(left)
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

func taggedDispatchDiscriminant(node *ast.Node) (*ast.Node, bool) {
	node = unwrapTaggedDispatchExpression(node)
	if node == nil || node.Kind != ast.KindPropertyAccessExpression {
		return nil, false
	}
	access := node.AsPropertyAccessExpression()
	if access == nil || access.Expression == nil || access.Name() == nil || access.Name().Text() != "_tag" {
		return nil, false
	}
	return node, true
}

func (tp *TypeParser) isTaggedDispatchDiscriminant(node *ast.Node, rootSymbol *ast.Symbol) bool {
	node, ok := taggedDispatchDiscriminant(node)
	if !ok || rootSymbol == nil {
		return false
	}
	access := node.AsPropertyAccessExpression()
	root := taggedDispatchAccessRoot(access.Expression)
	if root == nil || !sameSymbolReference(tp.checker, tp.GetSymbolAtLocation(root), rootSymbol) {
		return false
	}

	tagSymbol := tp.GetSymbolAtLocation(node)
	ownerType := tp.GetTypeAtLocation(access.Expression)
	if tagSymbol == nil || ownerType == nil {
		return false
	}
	if sameSymbolReference(tp.checker, tagSymbol, tp.checker.GetPropertyOfType(ownerType, "_tag")) {
		return true
	}
	for _, member := range tp.UnrollUnionMembers(ownerType) {
		if member != nil && sameSymbolReference(tp.checker, tagSymbol, tp.checker.GetPropertyOfType(member, "_tag")) {
			return true
		}
	}
	return false
}

func taggedDispatchAccessRoot(node *ast.Node) *ast.Node {
	node = unwrapTaggedDispatchExpression(node)
	for node != nil && node.Kind == ast.KindPropertyAccessExpression {
		access := node.AsPropertyAccessExpression()
		if access == nil || access.Expression == nil {
			return nil
		}
		node = unwrapTaggedDispatchExpression(access.Expression)
	}
	return node
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
