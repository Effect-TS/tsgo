package fixables

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var ForLoopPushToForEachFix = fixable.Fixable{
	Name:        "forLoopPushToForEach",
	Description: "Replace for-of push loop with Effect.forEach",
	ErrorCodes: []int32{
		tsdiag.Effect_forEach_expresses_this_for_of_loop_that_pushes_yielded_results_into_an_accumulator_more_directly_and_makes_ordering_and_concurrency_explicit_effect_forLoopPushToForEach.Code(),
	},
	FixIDs: []string{"forLoopPushToForEach_fix"},
	Run:    runForLoopPushToForEachFix,
}

func runForLoopPushToForEachFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	tp := ctx.TypeParser
	for _, match := range rules.AnalyzeForLoopPushToForEach(tp, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		forOf := match.Loop.AsForInOrOfStatement()
		binding := forOf.Initializer.AsVariableDeclarationList().Declarations.Nodes[0].Name()
		yielded := match.Flow.Subject().Node().AsYieldExpression().Expression
		accumulator := match.Declaration.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0].AsVariableDeclaration()
		// The fix moves binding and yielded into an arrow function, where a
		// yield of the generator cannot go, and turns the accumulator into the
		// result of Effect.forEach, which later mutations would no longer extend.
		if containsGeneratorYield(binding) || containsGeneratorYield(yielded) ||
			forLoopPushToForEachMutatedAfterLoop(tp, match.Loop, tp.GetSymbolAtLocation(accumulator.Name())) {
			continue
		}
		forEach := effectModuleMember(ctx, "forEach")
		reference := resolveModuleMember(ctx, match.Loop, typeparser.FindEffectModuleReference(sf), forEach)
		if reference == nil {
			continue
		}

		// The fix deletes one statement and replaces another, the loop; the
		// piping flow edits only rewrite steps inside a flow, so none applies.
		// The replacement is written as text: the printer drops the parentheses
		// of a synthesized single-identifier arrow parameter.
		callee := reference.path.String()
		if reference.viaModule {
			callee += "." + forEach.name
		}
		pointFreeCallee := forLoopPushToForEachPointFreeCallee(tp, ctx.Checker, binding, yielded)
		callback := forLoopPushToForEachNodeText(sf, pointFreeCallee)
		if pointFreeCallee == nil {
			body := forLoopPushToForEachNodeText(sf, yielded)
			// A concise arrow body starting with `{` would parse as a block.
			if strings.HasPrefix(body, "{") {
				body = "(" + body + ")"
			}
			callback = "(" + forLoopPushToForEachNodeText(sf, binding) + ") => " + body
		}
		declaration := "const " + forLoopPushToForEachNodeText(sf, accumulator.Name())
		if accumulator.Type != nil {
			declaration += ": " + forLoopPushToForEachNodeText(sf, accumulator.Type)
		}
		replacement := declaration + " = yield* " + callee + "(" + forLoopPushToForEachNodeText(sf, forOf.Expression) + ", " + callback + ")"
		if forLoopPushToForEachNeedsSemicolon(sf, match.Loop) {
			replacement += ";"
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.forEach",
			Run: func(tracker *rewriter.Tracker) {
				tracker.Delete(sf, match.Declaration)
				tracker.ReplaceRangeWithText(sf, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(scanner.GetTokenPosOfNode(match.Loop, sf, false)),
					End:   ctx.BytePosToLSPPosition(match.Loop.End()),
				}, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// forLoopPushToForEachMutatingMethods are the Array methods that change the
// array they are called on.
var forLoopPushToForEachMutatingMethods = map[string]bool{
	"push": true, "pop": true, "shift": true, "unshift": true, "splice": true,
	"sort": true, "reverse": true, "fill": true, "copyWithin": true,
}

// forLoopPushToForEachMutatedAfterLoop reports whether a statement after loop
// in the same block calls a mutating Array method on the accumulator, writes
// one of its elements or its length, or deletes one of its elements.
func forLoopPushToForEachMutatedAfterLoop(tp *typeparser.TypeParser, loop *ast.Node, symbol *ast.Symbol) bool {
	for _, reference := range tp.ReferencesToSymbol(loop.Parent, symbol).All() {
		if reference.Pos() < loop.End() {
			continue
		}
		parent := reference.Parent
		switch parent.Kind {
		case ast.KindPropertyAccessExpression:
			access := parent.AsPropertyAccessExpression()
			if access.Expression != reference {
				continue
			}
			member := access.Name().Text()
			if forLoopPushToForEachMutatingMethods[member] || member == "length" && ast.IsWriteAccess(parent) {
				return true
			}
		case ast.KindElementAccessExpression:
			if parent.AsElementAccessExpression().Expression != reference {
				continue
			}
			if ast.IsWriteAccess(parent) || parent.Parent != nil && parent.Parent.Kind == ast.KindDeleteExpression {
				return true
			}
		}
	}
	return false
}

// containsGeneratorYield reports whether node contains a yield expression of
// the enclosing generator, which could not move into an arrow function.
func containsGeneratorYield(node *ast.Node) bool {
	var visit func(*ast.Node) bool
	visit = func(n *ast.Node) bool {
		if n.Kind == ast.KindYieldExpression {
			return true
		}
		if ast.IsFunctionLike(n) {
			return false
		}
		//ets:ast-walk finds yield expressions of the enclosing generator, which are not flow steps
		return n.ForEachChild(visit)
	}
	return visit(node)
}

// forLoopPushToForEachPointFreeCallee returns f when yielded is a flow applying
// a single step f to the loop variable x, as in `f(x)`, `pipe(x, f)` or
// `x.pipe(f)`, and f is a plain identifier with a single call signature taking
// at most one parameter, so Effect.forEach passing (x, index) cannot reach a
// parameter the loop never filled. It returns nil when the fix has to keep an
// arrow around yielded.
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
	// No arguments leaves only call, pipe and pipeable steps: every other kind
	// keeps at least one argument besides the subject.
	isPlainCallee := func(transformation typeparser.PipingFlowTransformation) bool {
		callee := transformation.Callee()
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

// forLoopPushToForEachNeedsSemicolon reports whether the declaration replacing
// loop must end with a semicolon. A for statement delimits itself, while the
// declaration relies on automatic semicolon insertion: it keeps the semicolon
// that ended a statement body, and adds one when the next statement starts on
// the same line or with a token that would continue the expression.
func forLoopPushToForEachNeedsSemicolon(sf *ast.SourceFile, loop *ast.Node) bool {
	text := sf.Text()
	end := loop.End()
	if strings.HasSuffix(text[:end], ";") {
		return true
	}
	next := scanner.SkipTrivia(text, end)
	if next >= len(text) || text[next] == '}' || text[next] == ';' {
		return false
	}
	if !strings.ContainsAny(text[end:next], "\n\r\u2028\u2029") {
		return true
	}
	return strings.IndexByte("([`+-/<", text[next]) >= 0
}

func forLoopPushToForEachNodeText(sf *ast.SourceFile, node *ast.Node) string {
	if node == nil {
		return ""
	}
	return scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false)
}
