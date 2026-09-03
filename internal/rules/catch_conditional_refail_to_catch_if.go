package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// CatchConditionalRefailToCatchIf detects conditional catch handlers that
// recover in one branch and re-fail the untouched handler parameter in the
// other branch.
var CatchConditionalRefailToCatchIf = rule.Rule{
	Name:            "catchConditionalRefailToCatchIf",
	Group:           "style",
	Description:     "Suggests Effect.catchIf, Effect.catchCauseIf, or Effect.catchTag for conditional catch handlers that re-fail their untouched input",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_2_expresses_selective_recovery_more_directly_than_Effect_0_with_a_conditional_Effect_1_passthrough_effect_catchConditionalRefailToCatchIf.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchConditionalRefailToCatchIf(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_2_expresses_selective_recovery_more_directly_than_Effect_0_with_a_conditional_Effect_1_passthrough_effect_catchConditionalRefailToCatchIf,
				nil,
				match.CatchMethodName,
				match.FailMethodName,
				match.PreferredMethodName,
			)
		}
		return diagnostics
	},
}

type CatchConditionalRefailToCatchIfMatch struct {
	SourceFile          *ast.SourceFile
	Location            core.TextRange
	CatchMethodName     string
	FailMethodName      string
	PreferredMethodName string
}

type conditionalRefailCatchMethods struct {
	catchMethodName     string
	failMethodName      string
	preferredMethodName string
}

// AnalyzeCatchConditionalRefailToCatchIf finds exact Effect.catch and
// Effect.catchCause transformations whose handler is a single two-way
// conditional with one untouched-parameter re-fail branch.
func AnalyzeCatchConditionalRefailToCatchIf(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []CatchConditionalRefailToCatchIfMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []CatchConditionalRefailToCatchIfMatch
	seen := make(map[*ast.Node]struct{})
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			if transformation.Node == nil || transformation.Callee == nil {
				continue
			}
			if _, duplicate := seen[transformation.Node]; duplicate {
				continue
			}

			methods, ok := conditionalRefailMethods(tp, transformation.Callee)
			if !ok || len(transformation.Args) != 1 {
				continue
			}

			inputType := flow.Subject.OutType
			if index > 0 {
				inputType = flow.Transformations[index-1].OutType
			}
			if tp.StrictEffectType(inputType) == nil {
				continue
			}

			preferredMethodName, ok := analyzeConditionalRefailHandler(tp, c, transformation.Args[0], methods)
			if !ok {
				continue
			}

			seen[transformation.Node] = struct{}{}
			matches = append(matches, CatchConditionalRefailToCatchIfMatch{
				SourceFile:          sf,
				Location:            scanner.GetErrorRangeForNode(sf, transformation.Callee),
				CatchMethodName:     methods.catchMethodName,
				FailMethodName:      methods.failMethodName,
				PreferredMethodName: preferredMethodName,
			})
		}
	}

	return matches
}

func conditionalRefailMethods(tp *typeparser.TypeParser, callee *ast.Node) (conditionalRefailCatchMethods, bool) {
	switch {
	case tp.IsNodeReferenceToEffectModuleApi(callee, "catch"):
		return conditionalRefailCatchMethods{
			catchMethodName:     "catch",
			failMethodName:      "fail",
			preferredMethodName: "catchIf",
		}, true
	case tp.IsNodeReferenceToEffectModuleApi(callee, "catchCause"):
		return conditionalRefailCatchMethods{
			catchMethodName:     "catchCause",
			failMethodName:      "failCause",
			preferredMethodName: "catchCauseIf",
		}, true
	default:
		return conditionalRefailCatchMethods{}, false
	}
}

func analyzeConditionalRefailHandler(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	handlerNode *ast.Node,
	methods conditionalRefailCatchMethods,
) (string, bool) {
	dispatch := typeparser.ParseReturningDispatch(handlerNode)
	if dispatch == nil || len(dispatch.Params) != 1 || dispatch.Dispatch == nil ||
		len(dispatch.Dispatch.Branches) != 1 || dispatch.Dispatch.Fallback == nil {
		return "", false
	}
	parameter := dispatch.Params[0]
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return "", false
	}
	parameterDeclaration := parameter.AsParameterDeclaration()
	if parameterDeclaration == nil || parameterDeclaration.DotDotDotToken != nil || parameterDeclaration.Initializer != nil {
		return "", false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if parameterSymbol == nil || checker.Checker_isSymbolAssigned(c, parameterSymbol) {
		return "", false
	}

	// A recovered tag maps more precisely to catchTag; an inverted tag branch
	// still maps to catchIf.
	branch := dispatch.Dispatch.Branches[0]
	tagSubject := dispatch.Dispatch.CommonTagSubject(tp)
	if _, ok := resultDispatchTagValue(branch.Condition); tagSubject != nil && ok &&
		isResultDispatchTagReference(tp, c, tagSubject, parameterSymbol) {
		branchRefails := isConditionalRefailExpression(tp, c, branch.Result, parameterSymbol, methods.failMethodName)
		fallbackRefails := isConditionalRefailExpression(tp, c, dispatch.Dispatch.Fallback, parameterSymbol, methods.failMethodName)
		if branchRefails == fallbackRefails {
			return "", false
		}
		recovery := branch.Result
		preferred := methods.preferredMethodName
		if branchRefails {
			recovery = dispatch.Dispatch.Fallback
		} else if methods.catchMethodName == "catch" {
			preferred = "catchTag"
		}
		if !isEffectExpression(tp, recovery) {
			return "", false
		}
		return preferred, true
	}

	condition := branch.Condition.Subject
	if !isConditionalRefailPredicate(tp, c, condition, parameterSymbol) {
		return "", false
	}

	trueRefails := isConditionalRefailExpression(tp, c, branch.Result, parameterSymbol, methods.failMethodName)
	falseRefails := isConditionalRefailExpression(tp, c, dispatch.Dispatch.Fallback, parameterSymbol, methods.failMethodName)
	if trueRefails == falseRefails {
		return "", false
	}

	recovery := branch.Result
	if trueRefails {
		recovery = dispatch.Dispatch.Fallback
	}
	if !isEffectExpression(tp, recovery) {
		return "", false
	}

	return methods.preferredMethodName, true
}

func isConditionalRefailExpression(tp *typeparser.TypeParser, c *checker.Checker, expression *ast.Node, parameterSymbol *ast.Symbol, failMethodName string) bool {
	expression = ast.SkipParentheses(expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return false
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 ||
		!tp.IsNodeReferenceToEffectModuleApi(call.Expression, failMethodName) {
		return false
	}
	argument := ast.SkipParentheses(call.Arguments.Nodes[0])
	if argument == nil || argument.Kind != ast.KindIdentifier {
		return false
	}
	actualSymbol := tp.GetSymbolAtLocation(argument)
	return actualSymbol != nil && checker.Checker_getSymbolIfSameReference(c, actualSymbol, parameterSymbol) != nil
}

func isConditionalRefailPredicate(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, parameterSymbol *ast.Symbol) bool {
	node = unwrapTransparentExpression(node)
	if node == nil {
		return false
	}

	switch node.Kind {
	case ast.KindCallExpression, ast.KindPropertyAccessExpression, ast.KindElementAccessExpression, ast.KindTypeOfExpression:
		return conditionalRefailNodeContainsParameter(tp, c, node, parameterSymbol)
	case ast.KindPrefixUnaryExpression:
		prefix := node.AsPrefixUnaryExpression()
		return prefix != nil && prefix.Operator == ast.KindExclamationToken && isConditionalRefailPredicate(tp, c, prefix.Operand, parameterSymbol)
	case ast.KindBinaryExpression:
		binary := node.AsBinaryExpression()
		if binary == nil || binary.OperatorToken == nil {
			return false
		}
		switch binary.OperatorToken.Kind {
		case ast.KindAmpersandAmpersandToken, ast.KindBarBarToken:
			return isConditionalRefailPredicate(tp, c, binary.Left, parameterSymbol) || isConditionalRefailPredicate(tp, c, binary.Right, parameterSymbol)
		case ast.KindInstanceOfKeyword:
			return conditionalRefailNodeContainsParameter(tp, c, binary.Left, parameterSymbol)
		case ast.KindInKeyword:
			return conditionalRefailNodeContainsParameter(tp, c, binary.Right, parameterSymbol)
		case ast.KindEqualsEqualsToken, ast.KindEqualsEqualsEqualsToken,
			ast.KindExclamationEqualsToken, ast.KindExclamationEqualsEqualsToken,
			ast.KindLessThanToken, ast.KindLessThanEqualsToken,
			ast.KindGreaterThanToken, ast.KindGreaterThanEqualsToken:
			return conditionalRefailNodeContainsParameter(tp, c, node, parameterSymbol)
		}
	}

	return false
}

func conditionalRefailNodeContainsParameter(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, parameterSymbol *ast.Symbol) bool {
	if node == nil || parameterSymbol == nil {
		return false
	}
	found := false
	var visit func(*ast.Node) bool
	visit = func(current *ast.Node) bool {
		if current == nil || found {
			return found
		}
		if current.Kind == ast.KindIdentifier {
			actual := tp.GetSymbolAtLocation(current)
			if actual != nil && checker.Checker_getSymbolIfSameReference(c, actual, parameterSymbol) != nil {
				found = true
				return true
			}
		}
		current.ForEachChild(visit)
		return found
	}
	visit(node)
	return found
}
