package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

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

// ParseTaggedDispatch decodes a returning arrow function or function expression
// whose branches compare string-literal _tag values rooted at rootSymbol.
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
	parsed := ParseReturningDispatch(node)
	if parsed == nil {
		return nil
	}
	dispatch := &TaggedDispatch{Fallback: parsed.Fallback}
	seen := make(map[string]struct{})
	for _, sourceBranch := range parsed.Branches {
		branch, ok := parseTaggedReturningBranch(sourceBranch)
		if !ok || !appendTaggedDispatchBranch(dispatch, seen, branch) {
			return nil
		}
	}
	if len(dispatch.Branches) == 0 {
		return nil
	}
	return dispatch
}

func parseTaggedReturningBranch(source ReturningDispatchBranch) (TaggedDispatchBranch, bool) {
	if source.Test == nil || source.TestNode == nil || source.Result == nil {
		return TaggedDispatchBranch{}, false
	}
	if source.Discriminant == nil {
		branch, ok := parseTaggedDispatchComparison(source.Test)
		if !ok {
			return TaggedDispatchBranch{}, false
		}
		branch.Result = source.Result
		return branch, true
	}

	discriminant, ok := taggedDispatchDiscriminant(source.Discriminant)
	tagNode := unwrapReturningDispatchExpression(source.Test)
	if !ok || tagNode == nil || !ast.IsStringLiteral(tagNode) {
		return TaggedDispatchBranch{}, false
	}
	return TaggedDispatchBranch{
		Tag:          tagNode.AsStringLiteral().Text,
		TagNode:      tagNode,
		TestNode:     source.TestNode,
		Discriminant: discriminant,
		Result:       source.Result,
	}, true
}

func parseTaggedDispatchComparison(node *ast.Node) (TaggedDispatchBranch, bool) {
	node = unwrapReturningDispatchExpression(node)
	if node == nil || node.Kind != ast.KindBinaryExpression {
		return TaggedDispatchBranch{}, false
	}
	binary := node.AsBinaryExpression()
	if binary == nil || binary.Left == nil || binary.Right == nil || binary.OperatorToken == nil ||
		(binary.OperatorToken.Kind != ast.KindEqualsEqualsToken && binary.OperatorToken.Kind != ast.KindEqualsEqualsEqualsToken) {
		return TaggedDispatchBranch{}, false
	}

	left := unwrapReturningDispatchExpression(binary.Left)
	right := unwrapReturningDispatchExpression(binary.Right)
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
	node = unwrapReturningDispatchExpression(node)
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
	node = unwrapReturningDispatchExpression(node)
	for node != nil && node.Kind == ast.KindPropertyAccessExpression {
		access := node.AsPropertyAccessExpression()
		if access == nil || access.Expression == nil {
			return nil
		}
		node = unwrapReturningDispatchExpression(access.Expression)
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
