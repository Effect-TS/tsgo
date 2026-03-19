package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// StrictBooleanExpressions enforces that expressions in conditional positions
// are strictly boolean-typed, reporting non-boolean types as diagnostics.
var StrictBooleanExpressions = rule.Rule{
	Name:            "strictBooleanExpressions",
	Description:     "Enforces boolean types in conditional expressions for type safety",
	DefaultSeverity: etscore.SeverityOff,
	Codes:           []int32{tsdiag.Unexpected_0_type_in_condition_expected_strictly_a_boolean_instead_effect_strictBooleanExpressions.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		conditionChecks := make(map[*ast.Node]bool)

		// Breadth-first traversal matching the reference implementation
		nodeToVisit := make([]*ast.Node, 0)
		pushChild := func(child *ast.Node) bool {
			nodeToVisit = append(nodeToVisit, child)
			return false
		}
		ctx.SourceFile.AsNode().ForEachChild(pushChild)

		for len(nodeToVisit) > 0 {
			node := nodeToVisit[0]
			nodeToVisit = nodeToVisit[1:]

			// Enqueue children
			node.ForEachChild(pushChild)

			var nodesToCheck []*ast.Node

			switch node.Kind {
			case ast.KindIfStatement:
				conditionChecks[node] = true
				nodesToCheck = append(nodesToCheck, node.AsIfStatement().Expression)

			case ast.KindWhileStatement:
				conditionChecks[node] = true
				nodesToCheck = append(nodesToCheck, node.AsWhileStatement().Expression)

			case ast.KindConditionalExpression:
				conditionChecks[node] = true
				nodesToCheck = append(nodesToCheck, node.AsConditionalExpression().Condition)

			case ast.KindPrefixUnaryExpression:
				prefix := node.AsPrefixUnaryExpression()
				if prefix.Operator == ast.KindExclamationToken {
					conditionChecks[node] = true
					nodesToCheck = append(nodesToCheck, prefix.Operand)
				}

			case ast.KindBinaryExpression:
				binExpr := node.AsBinaryExpression()
				opKind := binExpr.OperatorToken.Kind
				if opKind == ast.KindBarBarToken || opKind == ast.KindAmpersandAmpersandToken {
					if conditionChecks[node.Parent] {
						conditionChecks[node] = true
					}
					nodesToCheck = append(nodesToCheck, binExpr.Left, binExpr.Right)
				}
			}

			for _, nodeToCheck := range nodesToCheck {
				if nodeToCheck == nil {
					continue
				}
				if !conditionChecks[nodeToCheck.Parent] {
					continue
				}

				nodeType := typeparser.GetTypeAtLocation(ctx.Checker, nodeToCheck)
				if nodeType == nil {
					continue
				}

				constrainedType := ctx.Checker.GetBaseConstraintOfType(nodeType)
				var typesToCheck []*checker.Type
				if constrainedType != nil {
					typesToCheck = append(typesToCheck, constrainedType)
				} else {
					typesToCheck = append(typesToCheck, nodeType)
				}

				for len(typesToCheck) > 0 {
					t := typesToCheck[len(typesToCheck)-1]
					typesToCheck = typesToCheck[:len(typesToCheck)-1]

					// Unroll union types
					if t.Flags()&checker.TypeFlagsUnion != 0 {
						typesToCheck = append(typesToCheck, typeparser.UnrollUnionMembers(t)...)
						continue
					}

					// Skip boolean and never types
					if t.Flags()&checker.TypeFlagsBoolean != 0 {
						continue
					}
					if t.Flags()&checker.TypeFlagsBooleanLiteral != 0 {
						continue
					}
					if t.Flags()&checker.TypeFlagsNever != 0 {
						continue
					}

					// Report the error
					typeName := ctx.Checker.TypeToString(t)
					diags = append(diags, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(nodeToCheck), tsdiag.Unexpected_0_type_in_condition_expected_strictly_a_boolean_instead_effect_strictBooleanExpressions, nil, typeName))
				}
			}
		}

		return diags
	},
}
