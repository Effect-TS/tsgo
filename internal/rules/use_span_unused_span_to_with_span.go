package rules

import (
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

var UseSpanUnusedSpanToWithSpan = rule.Rule{
	Name:            "useSpanUnusedSpanToWithSpan",
	Group:           "antipattern",
	Description:     "Suggests Effect.withSpan when an Effect.useSpan callback never uses its span, since useSpan does not parent inner spans",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_Effect_useSpan_callback_never_uses_the_span_it_receives_Effect_useSpan_does_not_make_that_span_the_parent_of_spans_created_inside_the_callback_so_they_will_not_nest_under_it_Effect_withSpan_does_and_removes_ParentSpan_from_the_requirements_effect_useSpanUnusedSpanToWithSpan.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUseSpanUnusedSpanToWithSpan(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.This_Effect_useSpan_callback_never_uses_the_span_it_receives_Effect_useSpan_does_not_make_that_span_the_parent_of_spans_created_inside_the_callback_so_they_will_not_nest_under_it_Effect_withSpan_does_and_removes_ParentSpan_from_the_requirements_effect_useSpanUnusedSpanToWithSpan, nil)
		}
		return diags
	},
}

// UseSpanUnusedSpanToWithSpanMatch is an Effect.useSpan call whose inline
// callback ignores its span. Receiver and Body are nil when the call cannot be
// rewritten to `Body.pipe(Receiver.withSpan(Name, Options))`.
type UseSpanUnusedSpanToWithSpanMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Call       *ast.Node
	Name       *ast.Node
	Options    *ast.Node
	Callback   *ast.Node
	Receiver   *ast.Node
	Body       *ast.Node
}

func AnalyzeUseSpanUnusedSpanToWithSpan(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []UseSpanUnusedSpanToWithSpanMatch {
	var matches []UseSpanUnusedSpanToWithSpanMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node.Kind == ast.KindCallExpression {
			if match, ok := analyzeUseSpanCall(tp, sf, node); ok {
				matches = append(matches, match)
			}
		}
		//ets:ast-walk finds Effect.useSpan calls, which are overloaded and not dual, so they never become flow steps
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

func analyzeUseSpanCall(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node) (UseSpanUnusedSpanToWithSpanMatch, bool) {
	call := node.AsCallExpression()
	if call.Arguments == nil {
		return UseSpanUnusedSpanToWithSpanMatch{}, false
	}
	args := call.Arguments.Nodes
	if len(args) != 2 && len(args) != 3 {
		return UseSpanUnusedSpanToWithSpanMatch{}, false
	}
	callback := ast.SkipParentheses(args[len(args)-1])
	if callback.Kind != ast.KindArrowFunction && callback.Kind != ast.KindFunctionExpression {
		return UseSpanUnusedSpanToWithSpanMatch{}, false
	}
	params := callback.Parameters()
	if len(params) > 1 || len(params) == 1 && !plainTapParameter(params[0].AsParameterDeclaration()) {
		return UseSpanUnusedSpanToWithSpanMatch{}, false
	}

	callee := ast.SkipParentheses(call.Expression)
	if !tp.IsNodeReferenceToEffectModuleApi(callee, "useSpan") {
		return UseSpanUnusedSpanToWithSpanMatch{}, false
	}
	body := callback.Body()
	if body == nil || callback.Kind == ast.KindFunctionExpression && readsArguments(body) ||
		len(params) == 1 && tp.AreParametersReferencedIn(callback, []*ast.Node{body}) {
		return UseSpanUnusedSpanToWithSpanMatch{}, false
	}

	match := UseSpanUnusedSpanToWithSpanMatch{
		SourceFile: sf,
		Location:   scanner.GetErrorRangeForNode(sf, callee),
		Call:       node,
		Name:       args[0],
		Callback:   callback,
	}
	if len(args) == 3 {
		match.Options = args[1]
	}
	match.Receiver, match.Body = useSpanRewriteParts(tp, node, callee, callback)
	return match, true
}

// useSpanRewriteParts returns the Effect module receiver and the callback's
// returned expression when the call can be rewritten to Effect.withSpan, or nil
// for both when moving the expression out of the callback could change what
// it observes or there is no module receiver to reuse.
func useSpanRewriteParts(tp *typeparser.TypeParser, call *ast.Node, callee *ast.Node, callback *ast.Node) (*ast.Node, *ast.Node) {
	if call.AsCallExpression().TypeArguments != nil || callee.Kind != ast.KindPropertyAccessExpression {
		return nil, nil
	}
	receiver := callee.AsPropertyAccessExpression().Expression
	if !tp.IsExpressionEffectModule(receiver) {
		return nil, nil
	}
	parsed := typeparser.ParseLazyExpression(callback, typeparser.LazyExpressionNone)
	if parsed == nil {
		return nil, nil
	}
	body := parsed.Expression()
	if callback.Kind == ast.KindFunctionExpression && usesFunctionExpressionBindings(tp, callback, readonly.New[*ast.Node](nil), body) {
		return nil, nil
	}
	if !tp.IsExpressionEvaluableAtLocation(body, call) {
		return nil, nil
	}
	return receiver, body
}

// readsArguments reports whether a function expression's body reads its own
// `arguments` object, through which it could reach the span, outside nested
// functions that bind their own.
func readsArguments(body *ast.Node) bool {
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		switch node.Kind {
		case ast.KindIdentifier:
			if node.Text() != "arguments" {
				return false
			}
			parent := node.Parent
			if parent != nil && ast.IsShorthandPropertyAssignment(parent) {
				// `{ arguments }` reads the object it names.
				return true
			}
			if ast.IsDeclarationName(node) {
				return false
			}
			return parent == nil || parent.Kind != ast.KindPropertyAccessExpression || parent.Name() != node
		case ast.KindFunctionExpression, ast.KindFunctionDeclaration, ast.KindMethodDeclaration,
			ast.KindGetAccessor, ast.KindSetAccessor, ast.KindConstructor:
			// A computed method name is evaluated in the enclosing scope.
			if name := node.Name(); name != nil && ast.IsComputedPropertyName(name) {
				return visit(name)
			}
			return false
		}
		//ets:ast-walk finds `arguments` identifier reads, which are not calls
		return node.ForEachChild(visit)
	}
	return visit(body)
}
