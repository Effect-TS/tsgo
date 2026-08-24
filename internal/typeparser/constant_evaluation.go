package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/evaluator"
	"github.com/microsoft/TypeScript/tsc/shim/jsnum"
)

// EvaluateConstantExpression evaluates the subset of JavaScript expressions
// that can be resolved statically without executing user code.
func (tp *TypeParser) EvaluateConstantExpression(node *ast.Node, location *ast.Node) evaluator.Result {
	if tp == nil || tp.checker == nil || node == nil {
		return evaluator.NewResult(nil, false, false, false)
	}
	if location == nil {
		location = node
	}

	numberSymbol := tp.checker.GetGlobalSymbol("Number", ast.SymbolFlagsValue, nil)
	var evaluate evaluator.Evaluator
	evaluate = evaluator.NewEvaluator(func(entity *ast.Node, entityLocation *ast.Node) evaluator.Result {
		var receiver *ast.Node
		var propertyName string
		switch entity.Kind {
		case ast.KindPropertyAccessExpression:
			access := entity.AsPropertyAccessExpression()
			if access != nil {
				receiver = access.Expression
				propertyName = access.Name().Text()
			}
		case ast.KindElementAccessExpression:
			access := entity.AsElementAccessExpression()
			if access != nil && ast.IsStringLiteralLike(access.ArgumentExpression) {
				receiver = access.Expression
				propertyName = access.ArgumentExpression.Text()
			}
		}

		if numberSymbol != nil && receiver != nil && tp.GetSymbolAtLocation(receiver) == numberSymbol {
			switch propertyName {
			case "POSITIVE_INFINITY":
				return evaluator.NewResult(jsnum.Inf(1), false, false, false)
			case "NEGATIVE_INFINITY":
				return evaluator.NewResult(jsnum.Inf(-1), false, false, false)
			case "NaN":
				return evaluator.NewResult(jsnum.NaN(), false, false, false)
			}
		}

		result := checker.Checker_evaluateEntity(tp.checker, entity, entityLocation)
		if result.Value != nil || entity.Kind != ast.KindIdentifier {
			return result
		}

		symbol := tp.GetSymbolAtLocation(entity)
		if symbol == nil || !checker.Checker_isConstantVariable(tp.checker, symbol) {
			return result
		}
		declaration := symbol.ValueDeclaration
		if declaration == nil || !ast.IsVariableDeclaration(declaration) || declaration.Type() != nil || declaration.Initializer() == nil ||
			entityLocation != nil && (declaration == entityLocation || !checker.Checker_isBlockScopedNameDeclaredBeforeUse(tp.checker, declaration, entityLocation)) {
			return result
		}

		constant := evaluate(declaration.Initializer(), declaration)
		resolvedOtherFiles := constant.ResolvedOtherFiles
		if entityLocation != nil && ast.GetSourceFileOfNode(entityLocation) != ast.GetSourceFileOfNode(declaration) {
			resolvedOtherFiles = true
		}
		return evaluator.NewResult(constant.Value, constant.IsSyntacticallyString, resolvedOtherFiles, true)
	}, ast.OEKParentheses)
	return evaluate(node, location)
}

// EvaluateConstantNumber evaluates a statically resolvable JavaScript numeric
// expression without executing user code.
func (tp *TypeParser) EvaluateConstantNumber(node *ast.Node, location *ast.Node) (float64, bool) {
	result := tp.EvaluateConstantExpression(node, location)
	number, ok := result.Value.(jsnum.Number)
	return float64(number), ok
}
