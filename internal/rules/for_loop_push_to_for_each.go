package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// ForLoopPushToForEach suggests Effect.forEach instead of a for-of loop inside
// an Effect generator that only pushes yielded Effect results into an empty
// accumulator declared just before the loop.
var ForLoopPushToForEach = rule.Rule{
	Name:            "forLoopPushToForEach",
	Group:           "style",
	Description:     "Suggests Effect.forEach instead of a for-of loop that pushes yielded Effect results into an accumulator",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_forEach_expresses_this_for_of_loop_that_pushes_yielded_results_into_an_accumulator_more_directly_and_makes_ordering_and_concurrency_explicit_effect_forLoopPushToForEach.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeForLoopPushToForEach(ctx.TypeParser, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				match.Location,
				tsdiag.Effect_forEach_expresses_this_for_of_loop_that_pushes_yielded_results_into_an_accumulator_more_directly_and_makes_ordering_and_concurrency_explicit_effect_forLoopPushToForEach,
				nil,
			)
		}
		return diagnostics
	},
}

// ForLoopPushToForEachMatch holds the nodes needed by the diagnostic and its fix.
type ForLoopPushToForEachMatch struct {
	Location core.TextRange
	// Loop is the for-of statement; its initializer holds the loop binding and
	// its expression the iterable.
	Loop *ast.Node
	// Declaration is the accumulator's `const acc = []` variable statement.
	Declaration *ast.Node
	// Flow is `yield* f(x)` piped into `acc.push`.
	Flow *typeparser.PipingFlow
}

// AnalyzeForLoopPushToForEach finds `for (const x of xs) acc.push(yield* f(x))`
// loops inside Effect generators where acc is `const acc = []`, declared right
// before the loop or separated from it only by other declarations.
//
// The push call is matched as a piping flow, `yield* f(x)` piped into
// `acc.push`; the loop and the accumulator declaration around it are
// statements, which flows do not represent, and are matched on the syntax tree.
func AnalyzeForLoopPushToForEach(tp *typeparser.TypeParser, sf *ast.SourceFile) []ForLoopPushToForEachMatch {
	if tp == nil || sf == nil {
		return nil
	}

	isYieldStar := func(subject typeparser.PipingFlowSubject) bool {
		node := subject.Node()
		if node == nil || node.Kind != ast.KindYieldExpression {
			return false
		}
		yield := node.AsYieldExpression()
		return yield.AsteriskToken != nil && yield.Expression != nil
	}
	// A plain call keeps push bound to the accumulator; pipe(x, acc.push) would not.
	isPush := func(transformation typeparser.PipingFlowTransformation) bool {
		callee := transformation.Callee()
		if transformation.Kind() != typeparser.TransformationKindCall || transformation.Args().Len() != 0 ||
			callee == nil || callee.Kind != ast.KindPropertyAccessExpression {
			return false
		}
		access := callee.AsPropertyAccessExpression()
		return access.Name().Text() == "push" && access.Expression.Kind == ast.KindIdentifier &&
			tp.IsNativeArrayMethodReference(callee, "push")
	}

	var matches []ForLoopPushToForEachMatch
	for _, flow := range tp.PipingFlows(sf, false).All() {
		if !flow.MatchesExactly(isYieldStar, isPush) {
			continue
		}
		if match, ok := analyzeForLoopPushToForEach(tp, sf, flow); ok {
			matches = append(matches, match)
		}
	}
	return matches
}

func analyzeForLoopPushToForEach(tp *typeparser.TypeParser, sf *ast.SourceFile, flow *typeparser.PipingFlow) (ForLoopPushToForEachMatch, bool) {
	loop := forLoopPushToForEachLoop(flow.Node())
	if loop == nil {
		return ForLoopPushToForEachMatch{}, false
	}
	forOf := loop.AsForInOrOfStatement()
	if forOf.AwaitModifier != nil || forOf.Initializer == nil || forOf.Initializer.Kind != ast.KindVariableDeclarationList || !ast.IsVarConst(forOf.Initializer) {
		return ForLoopPushToForEachMatch{}, false
	}
	loopDeclarations := forOf.Initializer.AsVariableDeclarationList().Declarations
	if loopDeclarations == nil || len(loopDeclarations.Nodes) != 1 || loopDeclarations.Nodes[0].Initializer() != nil {
		return ForLoopPushToForEachMatch{}, false
	}
	binding := loopDeclarations.Nodes[0].Name()

	receiver := flow.Transformations().At(0).Callee().AsPropertyAccessExpression().Expression
	yielded := flow.Subject().Node().AsYieldExpression().Expression

	if loop.Parent == nil || loop.Parent.Kind != ast.KindBlock {
		return ForLoopPushToForEachMatch{}, false
	}
	declaration, between := forLoopPushToForEachAccumulator(loop, receiver.Text())
	if declaration == nil {
		return ForLoopPushToForEachMatch{}, false
	}
	accumulator := declaration.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0]

	if tp.GetEffectContextFlags(loop)&typeparser.EffectContextFlagCanYieldEffect == 0 {
		return ForLoopPushToForEachMatch{}, false
	}

	accumulatorSymbol := tp.GetSymbolAtLocation(accumulator.Name())
	if accumulatorSymbol == nil || tp.GetSymbolAtLocation(receiver) != accumulatorSymbol {
		return ForLoopPushToForEachMatch{}, false
	}
	for _, node := range append([]*ast.Node{forOf.Expression, binding, yielded}, between...) {
		if tp.ReferencesToSymbol(node, accumulatorSymbol).Len() != 0 {
			return ForLoopPushToForEachMatch{}, false
		}
	}

	if tp.EffectTypeAtLocation(yielded) == nil {
		return ForLoopPushToForEachMatch{}, false
	}

	return ForLoopPushToForEachMatch{
		Location:    scanner.GetErrorRangeForNode(sf, loop),
		Loop:        loop,
		Declaration: declaration,
		Flow:        flow,
	}, true
}

// forLoopPushToForEachLoop returns the for-of statement whose body is only the
// push call, either as the statement itself or as the only statement of a block.
func forLoopPushToForEachLoop(push *ast.Node) *ast.Node {
	if ast.IsOptionalChain(push) {
		return nil
	}
	statement := push.Parent
	if statement == nil || statement.Kind != ast.KindExpressionStatement {
		return nil
	}
	body := statement
	if parent := statement.Parent; parent != nil && parent.Kind == ast.KindBlock {
		if len(parent.AsBlock().Statements.Nodes) != 1 {
			return nil
		}
		body = parent
	}
	loop := body.Parent
	if loop == nil || loop.Kind != ast.KindForOfStatement || loop.AsForInOrOfStatement().Statement != body {
		return nil
	}
	return loop
}

// forLoopPushToForEachAccumulator walks back from loop over variable statements
// only, and returns the `const name = []` statement declaring the accumulator
// together with the declarations passed on the way.
func forLoopPushToForEachAccumulator(loop *ast.Node, name string) (*ast.Node, []*ast.Node) {
	statements := loop.Parent.AsBlock().Statements.Nodes
	index := -1
	for i, statement := range statements {
		if statement == loop {
			index = i
			break
		}
	}
	var between []*ast.Node
	for i := index - 1; i >= 0; i-- {
		statement := statements[i]
		if statement.Kind != ast.KindVariableStatement {
			return nil, nil
		}
		list := statement.AsVariableStatement().DeclarationList
		declarations := list.AsVariableDeclarationList().Declarations.Nodes
		declaresName := false
		for _, declaration := range declarations {
			if declaration.Name().Kind == ast.KindIdentifier && declaration.Name().Text() == name {
				declaresName = true
				break
			}
		}
		if !declaresName {
			between = append(between, statement)
			continue
		}
		if len(declarations) != 1 || !ast.IsVarConst(list) {
			return nil, nil
		}
		initializer := declarations[0].Initializer()
		if initializer == nil || initializer.Kind != ast.KindArrayLiteralExpression ||
			len(initializer.AsArrayLiteralExpression().Elements.Nodes) != 0 {
			return nil, nil
		}
		return statement, between
	}
	return nil, nil
}
