package fixables

import (
	"slices"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var RunProvidedContextToRunWithFix = fixable.Fixable{
	Name:        "runProvidedContextToRunWith",
	Description: "Replace with the dedicated Effect run*With runner",
	ErrorCodes: []int32{
		tsdiag.Effect_0_of_Effect_1_re_implements_Effect_2_Use_Effect_2_context_effect_to_run_the_effect_with_the_context_directly_effect_runProvidedContextToRunWith.Code(),
	},
	FixIDs: []string{"runProvidedContextToRunWith_fix"},
	Run:    runRunProvidedContextToRunWithFix,
}

func runRunProvidedContextToRunWithFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeRunProvidedContextToRunWith(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Provide == nil || match.Runner == nil || match.Context == nil {
			continue
		}
		// Explicit runner type arguments have no verified rewrite: report only.
		if typeArguments := match.Runner.TypeArguments(); typeArguments != nil && len(typeArguments.Nodes) > 0 {
			continue
		}

		// run*With(context)(effect) evaluates the context before the effect;
		// only reorder when the context cannot observe or affect that work.
		if runProvidedContextReordersContext(match) && !orderInsensitiveContext(ctx, match.Context) {
			continue
		}
		// Only offer the fix when the generated reference resolves at the call site.
		replacement := effectModuleMember(ctx, match.ReplacementName)
		var reference *moduleMemberReference
		if match.EffectModuleNode == nil {
			if reference = resolveModuleMember(ctx, match.RunnerCallee, typeparser.FindEffectModuleReference(ctx.SourceFile), replacement); reference == nil {
				continue
			}
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				var callee *ast.Node
				if match.EffectModuleNode != nil {
					callee = tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(match.EffectModuleNode), nil, tracker.NewIdentifier(match.ReplacementName), ast.NodeFlagsNone)
				} else {
					callee = reference.expression(tracker, replacement)
				}
				// The provide step and the runner collapse into run*With(context),
				// applied to the provide step's input with the runner's options.
				tracker.CollapsePipingFlowTransformations(ctx.SourceFile, match.Flow, match.ProvideIndex, match.ProvideIndex+1, rewriter.PipingFlowTransformationReplacement{
					Callee:    callee,
					Arguments: tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.Context)}),
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// runProvidedContextReordersContext reports whether the fix moves the context
// ahead of the effect expression. A called runner originally evaluates its whole
// effect argument, the context last unless the provide step is the curried
// provideContext(context)(effect); a runner pipe step keeps the source order.
func runProvidedContextReordersContext(match rules.RunProvidedContextToRunWithMatch) bool {
	switch match.Runner.Kind() {
	case typeparser.TransformationKindCall, typeparser.TransformationKindCallWithOptionals:
		return match.Provide.Kind() != typeparser.TransformationKindCall
	}
	return false
}

// runProvidedContextConstructors are the Context constructors that only build a
// new Context from their arguments.
var runProvidedContextConstructors = []string{"empty", "make", "add", "merge", "mergeAll", "pick", "omit"}

// orderInsensitiveContext reports whether evaluating node earlier yields the
// same value and has no effect other code could observe: literals, const
// bindings, parameters that are never reassigned, and object or array literals
// and Context constructor calls built only from those.
//
// A binding is only a stable reference: its contents may still change, which
// is harmless where the reference itself is provided, because both forms read
// the Context when the effect runs. A constructor call reads the contents of
// its Context arguments when it is evaluated, so those must be fresh
// constructor calls. Tags only contribute their key and service values are
// stored by reference, so stable bindings are fine there.
func orderInsensitiveContext(ctx *fixable.Context, node *ast.Node) bool {
	if node == nil {
		return false
	}
	node = ast.SkipOuterExpressions(node, ast.OEKAll)
	switch node.Kind {
	case ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral, ast.KindNoSubstitutionTemplateLiteral,
		ast.KindTrueKeyword, ast.KindFalseKeyword, ast.KindNullKeyword:
		return true
	case ast.KindIdentifier:
		symbol := ctx.TypeParser.GetSymbolAtLocation(node)
		if symbol == nil {
			return false
		}
		// The value declaration, so a const merged with an interface still counts.
		target := checker.SkipAlias(symbol, ctx.Checker)
		if target == nil {
			return false
		}
		declaration := ast.TSGetSymbolValueDeclaration(target)
		if declaration == nil {
			return false
		}
		switch declaration.Kind {
		case ast.KindVariableDeclaration:
			return ast.IsVarConst(declaration)
		case ast.KindParameter:
			return !checker.Checker_isSymbolAssigned(ctx.Checker, target)
		}
		return false
	case ast.KindObjectLiteralExpression:
		for _, property := range node.AsObjectLiteralExpression().Properties.Nodes {
			switch property.Kind {
			case ast.KindPropertyAssignment:
				if name := property.Name(); name == nil || name.Kind == ast.KindComputedPropertyName ||
					!orderInsensitiveContext(ctx, property.AsPropertyAssignment().Initializer) {
					return false
				}
			case ast.KindShorthandPropertyAssignment:
				if !orderInsensitiveContext(ctx, property.Name()) {
					return false
				}
			default:
				return false
			}
		}
		return true
	case ast.KindArrayLiteralExpression:
		for _, element := range node.AsArrayLiteralExpression().Elements.Nodes {
			if !orderInsensitiveContext(ctx, element) {
				return false
			}
		}
		return true
	case ast.KindCallExpression:
		call := node.AsCallExpression()
		if call.QuestionDotToken != nil || !slices.ContainsFunc(runProvidedContextConstructors, func(name string) bool {
			return ctx.TypeParser.IsNodeReferenceToEffectContextModuleApi(call.Expression, name)
		}) {
			return false
		}
		for _, argument := range call.Arguments.Nodes {
			if ctx.TypeParser.ContextTypeAtLocation(argument) != nil &&
				ast.SkipOuterExpressions(argument, ast.OEKAll).Kind != ast.KindCallExpression {
				return false
			}
			if !orderInsensitiveContext(ctx, argument) {
				return false
			}
		}
		return true
	}
	return false
}
