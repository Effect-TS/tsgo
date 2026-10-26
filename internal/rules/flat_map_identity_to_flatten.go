package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/readonly"
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
	Description:     "Suggests Effect.flatten instead of Effect.flatMap when the callback is an identity function",
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

// AnalyzeFlatMapIdentityToFlatten finds Effect.flatMap steps whose callback
// returns its parameter unchanged, so the step only unwraps a nested effect.
func AnalyzeFlatMapIdentityToFlatten(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FlatMapIdentityToFlattenMatch {
	var matches []FlatMapIdentityToFlattenMatch
	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows.All() {
		for i, step := range flow.Transformations().All() {
			callee := step.Callee()
			args := step.Args()
			if step.Kind() == typeparser.TransformationKindCall && callee != nil && callee.Kind == ast.KindCallExpression {
				factory := callee.AsCallExpression()
				if factory.Arguments != nil && len(factory.Arguments.Nodes) == 1 && factory.TypeArguments == nil &&
					tp.IsNodeReferenceToEffectModuleApi(factory.Expression, "flatMap") {
					callee = factory.Expression
					args = readonly.New(factory.Arguments.Nodes)
				}
			}
			if callee == nil || args.Len() != 1 || step.TypeArguments() != nil ||
				callee.Flags&ast.NodeFlagsOptionalChain != 0 ||
				!tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") {
				continue
			}
			outer := tp.EffectType(flow.TransformationInputType(i))
			if outer == nil {
				continue
			}
			success := outer.A()
			if success == nil || success.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsNever) != 0 {
				continue
			}
			inner := tp.EffectType(success)
			if inner == nil || !isIdentityCallback(tp, args.At(0), success) {
				continue
			}
			// A callback typed through an annotated alias of identity can still widen the
			// result, so require the step to produce exactly what Effect.flatten would.
			output := tp.EffectType(step.OutType())
			if output == nil || !sameFlattenChannel(c, output.A(), inner.A()) ||
				!sameFlattenChannel(c, output.E(), outer.E(), inner.E()) ||
				!sameFlattenChannel(c, output.R(), outer.R(), inner.R()) {
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

// isIdentityCallback reports whether callback is Effect's identity export or a
// single-parameter function returning that parameter without changing its type.
func isIdentityCallback(tp *typeparser.TypeParser, callback *ast.Node, input *checker.Type) bool {
	if tp.IsNodeReferenceToEffectFunctionModuleApi(ast.SkipParentheses(callback), "identity") {
		return true
	}
	lazy := typeparser.ParseLazyExpression(callback, typeparser.LazyExpressionNone)
	if lazy == nil || lazy.Params().Len() != 1 || lazy.Node().Type() != nil {
		return false
	}
	parameter := lazy.Params().At(0).AsParameterDeclaration()
	if parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier || parameter.DotDotDotToken != nil ||
		parameter.Initializer != nil || parameter.QuestionToken != nil {
		return false
	}
	returned := ast.SkipParentheses(lazy.Expression())
	if returned.Kind != ast.KindIdentifier {
		return false
	}
	symbol := tp.GetSymbolAtLocation(parameter.Name())
	if symbol == nil || tp.GetSymbolAtLocation(returned) != symbol {
		return false
	}
	return parameter.Type == nil || tp.GetTypeAtLocation(parameter.Name()) == input
}

// sameFlattenChannel reports whether got is mutually assignable with the union
// of parts, comparing union constituents so no union type has to be built.
// any is assignable both ways, so got must be any exactly when a part is.
func sameFlattenChannel(c *checker.Checker, got *checker.Type, parts ...*checker.Type) bool {
	if got == nil || slices.Contains(parts, nil) {
		return false
	}
	isAny := func(t *checker.Type) bool { return t.Flags()&checker.TypeFlagsAny != 0 }
	if isAny(got) != slices.ContainsFunc(parts, isAny) {
		return false
	}
	for _, part := range parts {
		if !c.IsTypeAssignableTo(part, got) {
			return false
		}
	}
	constituents := []*checker.Type{got}
	if got.Flags()&checker.TypeFlagsUnion != 0 {
		constituents = got.Types()
	}
	for _, constituent := range constituents {
		if !slices.ContainsFunc(parts, func(part *checker.Type) bool { return c.IsTypeAssignableTo(constituent, part) }) {
			return false
		}
	}
	return true
}
