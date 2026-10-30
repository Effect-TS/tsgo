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

// ForLoopPushToForEach suggests Effect.forEach instead of a for-of loop inside
// an Effect generator that only pushes yielded Effect results into an empty
// accumulator declared just before the loop.
var ForLoopPushToForEach = rule.Rule{
	Name:            "forLoopPushToForEach",
	Group:           "style",
	Description:     "Suggests Effect.forEach instead of a for-of loop that pushes yielded Effect results into an accumulator",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_forEach_expresses_this_for_of_loop_that_pushes_yielded_results_into_an_accumulator_more_directly_and_makes_ordering_and_concurrency_explicit_effect_forLoopPushToForEach.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeForLoopPushToForEach(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
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
	Loop     *ast.Node
	// Declaration is the accumulator's variable statement.
	Declaration     *ast.Node
	AccumulatorName *ast.Node
	// AccumulatorType is the accumulator's type annotation, or nil.
	AccumulatorType *ast.Node
	Iterable        *ast.Node
	// Binding is the loop variable's name, possibly a binding pattern.
	Binding *ast.Node
	Yielded *ast.Node
	// PointFreeCallee is the callee Effect.forEach can receive directly, or nil
	// when the fix has to keep an arrow around Yielded.
	PointFreeCallee *ast.Node
	// CanFix reports whether replacing the accumulator with the result of
	// Effect.forEach is safe: the accumulator is not mutated after the loop and
	// Binding and Yielded, which the fix moves into an arrow function, contain
	// no yield of the generator.
	CanFix bool
}

// AnalyzeForLoopPushToForEach finds `for (const x of xs) acc.push(yield* f(x))`
// loops inside Effect generators where acc is `const acc = []`, declared right
// before the loop or separated from it only by other declarations.
//
// The push call is matched as a piping flow, `yield* f(x)` piped into
// `acc.push`; the loop and the accumulator declaration around it are
// statements, which flows do not represent, and are matched on the syntax tree.
func AnalyzeForLoopPushToForEach(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ForLoopPushToForEachMatch {
	if tp == nil || c == nil || sf == nil {
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
		if match, ok := analyzeForLoopPushToForEach(tp, c, sf, flow); ok {
			matches = append(matches, match)
		}
	}
	return matches
}

func analyzeForLoopPushToForEach(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, flow *typeparser.PipingFlow) (ForLoopPushToForEachMatch, bool) {
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
	accumulator := declaration.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0].AsVariableDeclaration()

	if tp.GetEffectContextFlags(loop)&typeparser.EffectContextFlagCanYieldEffect == 0 {
		return ForLoopPushToForEachMatch{}, false
	}

	accumulatorSymbol := tp.GetSymbolAtLocation(accumulator.Name())
	if accumulatorSymbol == nil || tp.GetSymbolAtLocation(receiver) != accumulatorSymbol {
		return ForLoopPushToForEachMatch{}, false
	}
	name := receiver.Text()
	if referencesSymbol(tp, c, forOf.Expression, name, accumulatorSymbol) ||
		referencesSymbol(tp, c, binding, name, accumulatorSymbol) ||
		referencesSymbol(tp, c, yielded, name, accumulatorSymbol) {
		return ForLoopPushToForEachMatch{}, false
	}
	for _, statement := range between {
		if referencesSymbol(tp, c, statement, name, accumulatorSymbol) {
			return ForLoopPushToForEachMatch{}, false
		}
	}

	if tp.EffectTypeAtLocation(yielded) == nil {
		return ForLoopPushToForEachMatch{}, false
	}

	return ForLoopPushToForEachMatch{
		Location:        scanner.GetErrorRangeForNode(sf, loop),
		Loop:            loop,
		Declaration:     declaration,
		AccumulatorName: accumulator.Name(),
		AccumulatorType: accumulator.Type,
		Iterable:        forOf.Expression,
		Binding:         binding,
		Yielded:         yielded,
		PointFreeCallee: forLoopPushToForEachPointFreeCallee(tp, c, binding, yielded),
		CanFix:          !mutatedAfterLoop(tp, loop, name, accumulatorSymbol) && !containsYield(binding) && !containsYield(yielded),
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

// referencesSymbol reports whether node contains an identifier, or a shorthand
// property, that resolves to symbol. Identifiers are compared by name first so
// most nodes skip the symbol lookup.
func referencesSymbol(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, name string, symbol *ast.Symbol) bool {
	var visit func(*ast.Node) bool
	visit = func(n *ast.Node) bool {
		switch n.Kind {
		case ast.KindShorthandPropertyAssignment:
			if n.Name().Text() == name && c.GetShorthandAssignmentValueSymbol(n) == symbol {
				return true
			}
		case ast.KindIdentifier:
			return n.Text() == name && tp.GetSymbolAtLocation(n) == symbol
		}
		return n.ForEachChild(visit)
	}
	return node != nil && visit(node)
}

// forLoopPushToForEachMutatingMethods are the Array methods that change the
// array they are called on.
var forLoopPushToForEachMutatingMethods = map[string]bool{
	"push": true, "pop": true, "shift": true, "unshift": true, "splice": true,
	"sort": true, "reverse": true, "fill": true, "copyWithin": true,
}

// mutatedAfterLoop reports whether a statement after loop in the same block
// calls a mutating Array method on the accumulator, writes one of its
// elements or its length, or deletes one of its elements.
func mutatedAfterLoop(tp *typeparser.TypeParser, loop *ast.Node, name string, symbol *ast.Symbol) bool {
	isMutation := func(reference *ast.Node) bool {
		parent := reference.Parent
		if parent == nil {
			return false
		}
		switch parent.Kind {
		case ast.KindPropertyAccessExpression:
			access := parent.AsPropertyAccessExpression()
			if access.Expression != reference {
				return false
			}
			member := access.Name().Text()
			return forLoopPushToForEachMutatingMethods[member] || member == "length" && ast.IsWriteAccess(parent)
		case ast.KindElementAccessExpression:
			if parent.AsElementAccessExpression().Expression != reference {
				return false
			}
			return ast.IsWriteAccess(parent) || parent.Parent != nil && parent.Parent.Kind == ast.KindDeleteExpression
		}
		return false
	}
	var visit func(*ast.Node) bool
	visit = func(n *ast.Node) bool {
		if n.Kind == ast.KindIdentifier {
			return n.Text() == name && tp.GetSymbolAtLocation(n) == symbol && isMutation(n)
		}
		return n.ForEachChild(visit)
	}
	after := false
	for _, statement := range loop.Parent.AsBlock().Statements.Nodes {
		if statement == loop {
			after = true
			continue
		}
		if after && visit(statement) {
			return true
		}
	}
	return false
}

// containsYield reports whether node contains a yield expression of the
// enclosing generator, which could not move into an arrow function.
func containsYield(node *ast.Node) bool {
	var visit func(*ast.Node) bool
	visit = func(n *ast.Node) bool {
		if n.Kind == ast.KindYieldExpression {
			return true
		}
		if ast.IsFunctionLike(n) {
			return false
		}
		return n.ForEachChild(visit)
	}
	return visit(node)
}

// forLoopPushToForEachPointFreeCallee returns f when yielded is a flow applying
// a single step f to the loop variable x, as in `f(x)`, `pipe(x, f)` or
// `x.pipe(f)`, and f is a plain identifier with a single call signature taking
// at most one parameter, so Effect.forEach passing (x, index) cannot reach a
// parameter the loop never filled.
func forLoopPushToForEachPointFreeCallee(tp *typeparser.TypeParser, c *checker.Checker, binding *ast.Node, yielded *ast.Node) *ast.Node {
	if binding.Kind != ast.KindIdentifier {
		return nil
	}
	bindingSymbol := tp.GetSymbolAtLocation(binding)
	if bindingSymbol == nil {
		return nil
	}
	isBinding := func(subject typeparser.PipingFlowSubject) bool {
		return subject.Node() != nil && subject.Node().Kind == ast.KindIdentifier && tp.GetSymbolAtLocation(subject.Node()) == bindingSymbol
	}
	isPlainCallee := func(transformation typeparser.PipingFlowTransformation) bool {
		callee := transformation.Callee()
		switch transformation.Kind() {
		case typeparser.TransformationKindCall, typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
		default:
			return false
		}
		return callee != nil && callee.Kind == ast.KindIdentifier && transformation.Args().Len() == 0 &&
			(transformation.TypeArguments() == nil || len(transformation.TypeArguments().Nodes) == 0) &&
			tp.GetSymbolAtLocation(callee) != bindingSymbol
	}
	flow := tp.LongestPipingFlowAt(yielded, false)
	if flow == nil || flow.Node() != ast.SkipParentheses(yielded) || !flow.MatchesExactly(isBinding, isPlainCallee) {
		return nil
	}
	if ast.IsOptionalChain(flow.Node()) {
		return nil
	}
	callee := flow.Transformations().At(0).Callee()
	calleeType := tp.GetTypeAtLocation(callee)
	if calleeType == nil {
		return nil
	}
	signatures := c.GetSignaturesOfType(calleeType, checker.SignatureKindCall)
	if len(signatures) != 1 || signatures[0].HasRestParameter() || len(signatures[0].Parameters()) > 1 {
		return nil
	}
	return callee
}
