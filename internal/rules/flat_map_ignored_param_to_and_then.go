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

// FlatMapIgnoredParamToAndThen suggests using Effect.andThen instead of Effect.flatMap
// when the callback ignores its parameter and returns a pre-existing effect.
var FlatMapIgnoredParamToAndThen = rule.Rule{
	Name:            "flatMapIgnoredParamToAndThen",
	Group:           "style",
	Description:     "Suggests using Effect.andThen instead of Effect.flatMap when the callback ignores its parameter and returns a pre-existing effect",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_andThen_expresses_sequencing_effects_directly_when_the_callback_parameter_is_ignored_effect_flatMapIgnoredParamToAndThen.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapIgnoredParamToAndThen(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_andThen_expresses_sequencing_effects_directly_when_the_callback_parameter_is_ignored_effect_flatMapIgnoredParamToAndThen,
				nil,
			)
		}
		return diags
	},
}

// FlatMapIgnoredParamToAndThenMatch holds the matched AST nodes for reporting.
type FlatMapIgnoredParamToAndThenMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	Callee           *ast.Node
	Callback         *ast.Node
	EffectExpression *ast.Node
}

// AnalyzeFlatMapIgnoredParamToAndThen finds calls to Effect.flatMap whose callback
// ignores its parameter and returns a pre-existing effect.
func AnalyzeFlatMapIgnoredParamToAndThen(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FlatMapIgnoredParamToAndThenMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []FlatMapIgnoredParamToAndThenMatch
	visitedCallees := make(map[core.TextRange]bool)

	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows {
		for _, transformation := range flow.Transformations {
			callee := transformation.Callee
			args := transformation.Args

			if callee == nil || len(args) == 0 {
				continue
			}

			// Must resolve to Effect.flatMap
			if !tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") {
				continue
			}

			loc := scanner.GetErrorRangeForNode(sf, callee)
			if visitedCallees[loc] {
				continue
			}

			callbackNode := args[0]
			lazy := typeparser.ParseLazyExpression(callbackNode, typeparser.LazyExpressionNone)
			if lazy == nil || lazy.Node == nil || lazy.Node.Kind != ast.KindArrowFunction {
				continue
			}

			// Reject block bodies: must be a plain expression body
			if lazy.Body == nil || lazy.Body.Kind == ast.KindBlock {
				continue
			}

			// Check parameter(s): zero parameters, or a single parameter that is never referenced
			params := typeparser.GetFunctionLikeParameters(lazy.Node)
			paramCount := 0
			if params != nil {
				paramCount = len(params.Nodes)
			}
			if paramCount > 1 {
				continue
			}

			if paramCount == 1 {
				paramDecl := params.Nodes[0].AsParameterDeclaration()
				if paramDecl == nil || paramDecl.Name() == nil || paramDecl.Name().Kind != ast.KindIdentifier ||
					paramDecl.Initializer != nil || paramDecl.DotDotDotToken != nil {
					continue
				}
				paramSymbol := tp.GetSymbolAtLocation(paramDecl.Name())
				if paramSymbol == nil {
					continue
				}
				if parameterIsReferenced(tp, c, paramSymbol, lazy.Body) {
					continue
				}
			}

			// Body must be a plain identifier or property access chain (no calls, no new expressions)
			bodyExpr := ast.SkipParentheses(lazy.Expression)
			if !isPlainIdentifierOrPropertyAccess(bodyExpr) {
				continue
			}

			// Body type must be assignable to Effect (including subtypes like Exit)
			bodyType := tp.GetTypeAtLocation(bodyExpr)
			if !isAssignableToEffect(tp, bodyType) {
				continue
			}

			visitedCallees[loc] = true
			matches = append(matches, FlatMapIgnoredParamToAndThenMatch{
				SourceFile:       sf,
				Location:         loc,
				Callee:           callee,
				Callback:         callbackNode,
				EffectExpression: bodyExpr,
			})
		}
	}

	return matches
}

// parameterIsReferenced checks if the given parameter symbol is referenced in the node tree.
func parameterIsReferenced(tp *typeparser.TypeParser, c *checker.Checker, paramSymbol *ast.Symbol, body *ast.Node) bool {
	var usesParameter func(node *ast.Node) bool
	usesParameter = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindShorthandPropertyAssignment && c.GetShorthandAssignmentValueSymbol(node) == paramSymbol {
			return true
		}
		if node.Kind == ast.KindIdentifier {
			sym := tp.GetSymbolAtLocation(node)
			if sym != nil && (sym == paramSymbol || checker.Checker_getSymbolIfSameReference(c, sym, paramSymbol) != nil) {
				return true
			}
		}
		return node.ForEachChild(usesParameter)
	}
	return usesParameter(body)
}

// isPlainIdentifierOrPropertyAccess checks that an expression is strictly an identifier
// or a chain of property accesses rooted at an identifier or `this`.
// Calls, new expressions, element access, binary expressions, and optional chaining are excluded.
func isPlainIdentifierOrPropertyAccess(node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindIdentifier:
		return true
	case ast.KindPropertyAccessExpression:
		pae := node.AsPropertyAccessExpression()
		if pae == nil || pae.QuestionDotToken != nil {
			return false
		}
		if pae.Name() == nil || pae.Name().Kind != ast.KindIdentifier {
			return false
		}
		return isAllowedPropertyAccessTarget(pae.Expression)
	default:
		return false
	}
}

func isAllowedPropertyAccessTarget(node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindIdentifier, ast.KindThisKeyword:
		return true
	case ast.KindPropertyAccessExpression:
		pae := node.AsPropertyAccessExpression()
		if pae == nil || pae.QuestionDotToken != nil {
			return false
		}
		if pae.Name() == nil || pae.Name().Kind != ast.KindIdentifier {
			return false
		}
		return isAllowedPropertyAccessTarget(pae.Expression)
	default:
		return false
	}
}

// isAssignableToEffect checks whether all constituents of the type (unrolling unions)
// satisfy the Effect variance interface (Effect<A, E, R> or subtypes like Exit<A, E>).
func isAssignableToEffect(tp *typeparser.TypeParser, t *checker.Type) bool {
	if tp == nil || t == nil {
		return false
	}
	members := tp.UnrollUnionMembers(t)
	if len(members) == 0 {
		return false
	}
	for _, m := range members {
		if !tp.IsEffectType(m) {
			return false
		}
	}
	return true
}
