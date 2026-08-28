package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// CommonTagSubject returns the common subject of a tag-based dispatch. It
// returns nil when any branch is not tag-based or when the tag subjects do not
// refer to the same expression by symbol.
func (dispatch *ResultDispatch) CommonTagSubject(tp *TypeParser) *ast.Node {
	if dispatch == nil || tp == nil || tp.checker == nil || len(dispatch.Branches) == 0 {
		return nil
	}

	common := dispatch.Branches[0].Condition.TagSubject
	if common == nil || dispatch.Branches[0].Condition.TagValue == nil {
		return nil
	}
	for _, branch := range dispatch.Branches[1:] {
		condition := branch.Condition
		if condition.TagSubject == nil || condition.TagValue == nil ||
			!tp.sameResultDispatchReference(common, condition.TagSubject) {
			return nil
		}
	}
	return common
}

func (tp *TypeParser) sameResultDispatchReference(left *ast.Node, right *ast.Node) bool {
	left = unwrapResultDispatchExpression(left)
	right = unwrapResultDispatchExpression(right)
	if left == nil || right == nil || left.Kind != right.Kind {
		return false
	}
	if left == right {
		return true
	}

	switch left.Kind {
	case ast.KindIdentifier:
		return sameSymbolReference(tp.checker, tp.GetSymbolAtLocation(left), tp.GetSymbolAtLocation(right))
	case ast.KindPropertyAccessExpression:
		leftAccess := left.AsPropertyAccessExpression()
		rightAccess := right.AsPropertyAccessExpression()
		return leftAccess != nil && rightAccess != nil &&
			leftAccess.Expression != nil && rightAccess.Expression != nil &&
			sameSymbolReference(tp.checker, tp.GetSymbolAtLocation(left), tp.GetSymbolAtLocation(right)) &&
			tp.sameResultDispatchReference(leftAccess.Expression, rightAccess.Expression)
	case ast.KindThisKeyword, ast.KindSuperKeyword:
		return true
	default:
		return false
	}
}
