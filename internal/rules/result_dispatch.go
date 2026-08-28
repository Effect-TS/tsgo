package rules

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func resultDispatchTagValue(condition typeparser.DispatchCondition) (string, bool) {
	value := unwrapTransparentExpression(condition.TagValue)
	if condition.TagSubject != nil && value != nil && ast.IsStringLiteral(value) {
		return value.AsStringLiteral().Text, true
	}
	return "", false
}

func isResultDispatchTagReference(tp *typeparser.TypeParser, c *checker.Checker, tagSubject *ast.Node, rootSymbol *ast.Symbol) bool {
	tagSubject = unwrapTransparentExpression(tagSubject)
	if tagSubject == nil || rootSymbol == nil {
		return false
	}

	root := tagSubject
	for {
		root = unwrapTransparentExpression(root)
		if root == nil || root.Kind != ast.KindPropertyAccessExpression {
			break
		}
		property := root.AsPropertyAccessExpression()
		if property == nil || property.Expression == nil {
			return false
		}
		root = property.Expression
	}
	if root == nil || !sameCatchReasonSymbol(tp, c, root, rootSymbol) {
		return false
	}

	ownerType := tp.GetTypeAtLocation(tagSubject)
	if ownerType == nil {
		return false
	}
	if c.GetPropertyOfType(ownerType, "_tag") != nil {
		return true
	}
	for _, member := range tp.UnrollUnionMembers(ownerType) {
		if member != nil && c.GetPropertyOfType(member, "_tag") != nil {
			return true
		}
	}
	return false
}
