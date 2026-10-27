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

var FlatMapIdentityToFlatten = rule.Rule{
	Name:            "flatMapIdentityToFlatten",
	Group:           "style",
	Description:     "Suggests Effect.flatten instead of Effect.flatMap when the callback is Effect's identity export referenced directly or an unannotated function returning its parameter",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_flatten_expresses_this_unwrapping_more_directly_than_Effect_flatMap_with_an_identity_callback_effect_flatMapIdentityToFlatten.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapIdentityToFlatten(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_flatten_expresses_this_unwrapping_more_directly_than_Effect_flatMap_with_an_identity_callback_effect_flatMapIdentityToFlatten, nil)
		}
		return diagnostics
	},
}

type FlatMapIdentityToFlattenMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	Callee         *ast.Node
	Transformation *typeparser.PipingFlowTransformation
	// EffectModuleNode is the receiver of Callee when it is an Effect module
	// property access, nil when flatMap is referenced without a module.
	EffectModuleNode *ast.Node
}

// AnalyzeFlatMapIdentityToFlatten finds Effect.flatMap steps whose callback is
// Effect's identity export referenced directly, or an unannotated arrow
// function or function expression returning its own parameter.
func AnalyzeFlatMapIdentityToFlatten(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []FlatMapIdentityToFlattenMatch {
	var matches []FlatMapIdentityToFlattenMatch
	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows.All() {
		for i, step := range flow.Transformations().All() {
			callee := step.Callee()
			args := step.Args()
			if callee == nil || args.Len() != 1 || step.TypeArguments() != nil ||
				callee.Flags&ast.NodeFlagsOptionalChain != 0 ||
				!tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") ||
				!isIdentityCallback(tp, args.At(0)) {
				continue
			}
			outer := tp.EffectType(flow.TransformationInputType(i))
			if outer == nil {
				continue
			}
			success := outer.A()
			if success == nil || success.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsNever) != 0 ||
				tp.EffectType(success) == nil {
				continue
			}
			var effectModuleNode *ast.Node
			if callee.Kind == ast.KindPropertyAccessExpression {
				receiver := callee.AsPropertyAccessExpression().Expression
				if tp.IsExpressionEffectModule(receiver) {
					effectModuleNode = receiver
				}
			}
			transformation := step
			matches = append(matches, FlatMapIdentityToFlattenMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, callee),
				Callee:           callee,
				Transformation:   &transformation,
				EffectModuleNode: effectModuleNode,
			})
		}
	}
	return matches
}

// isIdentityCallback reports whether callback names Effect's identity export
// without a local variable in between, or is an arrow function or function
// expression without type annotations whose body returns its single parameter. Annotations and typed
// aliases can change the flatMap result type, so they never match.
func isIdentityCallback(tp *typeparser.TypeParser, callback *ast.Node) bool {
	callback = ast.SkipParentheses(callback)
	if tp.IsNodeDirectReferenceToEffectFunctionModuleApi(callback, "identity") {
		return true
	}
	lazy := typeparser.ParseLazyExpression(callback, typeparser.LazyExpressionNone)
	if lazy == nil || lazy.Params().Len() != 1 || lazy.Node().Type() != nil {
		return false
	}
	parameter := lazy.Params().At(0).AsParameterDeclaration()
	if parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier || parameter.Type != nil ||
		parameter.DotDotDotToken != nil || parameter.Initializer != nil || parameter.QuestionToken != nil {
		return false
	}
	returned := ast.SkipParentheses(lazy.Expression())
	if returned.Kind != ast.KindIdentifier {
		return false
	}
	symbol := tp.GetSymbolAtLocation(parameter.Name())
	return symbol != nil && tp.GetSymbolAtLocation(returned) == symbol
}
