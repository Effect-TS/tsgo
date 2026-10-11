package fixables

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var FlatMapToTapFix = fixable.Fixable{
	Name:        "flatMapToTap",
	Description: "Replace Effect.flatMap or Effect.andThen with Effect.tap",
	ErrorCodes: []int32{
		tsdiag.Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap.Code(),
	},
	FixIDs: []string{"flatMapToTap_fix"},
	Run:    runFlatMapToTapFix,
}

func runFlatMapToTapFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFlatMapToTap(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		remove := tapReturnRemoval(match.SourceFile, match.ReturnFlow)
		if remove == nil {
			continue
		}
		target, receiver := tapReference(ctx, match.Callee)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.tap",
			Run: func(tracker *rewriter.Tracker) {
				remove(tracker)
				if match.Callee.Kind == ast.KindPropertyAccessExpression && target == match.Callee.AsPropertyAccessExpression().Expression {
					tracker.ReplaceNode(match.SourceFile, match.Callee.AsPropertyAccessExpression().Name(), tracker.NewIdentifier("tap"), nil)
					return
				}
				var replacement *ast.Node
				if receiver {
					replacement = effectModuleMethod(tracker, match.SourceFile, target, "tap")
				} else {
					replacement = tracker.NewIdentifier(target.Text())
				}
				tracker.ReplaceNode(match.SourceFile, match.Callee, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func tapReference(ctx *fixable.Context, callee *ast.Node) (*ast.Node, bool) {
	if callee.Kind == ast.KindPropertyAccessExpression {
		receiver := callee.AsPropertyAccessExpression().Expression
		if ctx.TypeParser.IsExpressionEffectModule(receiver) && tapModuleReference(ctx, receiver) {
			return receiver, true
		}
	}
	symbols := ctx.Checker.GetSymbolsInScope(callee, ast.SymbolFlagsValue|ast.SymbolFlagsAlias)
	slices.SortFunc(symbols, func(left, right *ast.Symbol) int {
		return strings.Compare(left.Name, right.Name)
	})
	for _, symbol := range symbols {
		if len(symbol.Declarations) != 1 || checker.Checker_isSymbolAssigned(ctx.Checker, symbol) {
			continue
		}
		declaration := symbol.Declarations[0]
		if ast.IsPartOfTypeOnlyImportOrExportDeclaration(declaration) {
			continue
		}
		name := declaration.Name()
		if name == nil || name.Kind != ast.KindIdentifier {
			continue
		}
		reference := name
		if declaration.Kind == ast.KindVariableDeclaration {
			if declaration.Parent == nil || declaration.Parent.Flags&ast.NodeFlagsConst == 0 || declaration.End() > callee.Pos() {
				continue
			}
			reference = declaration.AsVariableDeclaration().Initializer
			if ctx.TypeParser.GetTypeAtLocation(name) != ctx.TypeParser.GetTypeAtLocation(reference) {
				continue
			}
		}
		if ctx.TypeParser.IsNodeReferenceToEffectModuleApi(reference, "tap") {
			return name, false
		}
		if ctx.TypeParser.IsExpressionEffectModule(reference) && tapModuleReference(ctx, name) {
			return name, true
		}
	}
	return nil, false
}

func tapModuleReference(ctx *fixable.Context, receiver *ast.Node) bool {
	tap := ctx.Checker.GetPropertyOfType(ctx.TypeParser.GetTypeAtLocation(receiver), "tap")
	return tap != nil && tap.ValueDeclaration != nil && ctx.TypeParser.IsNodeReferenceToEffectModuleApi(tap.ValueDeclaration.Name(), "tap")
}

func tapReturnRemoval(sf *ast.SourceFile, flow *typeparser.PipingFlow) func(*rewriter.Tracker) {
	last := flow.Transformations().Len() - 1
	terminal := flow.Transformations().At(last)
	switch terminal.Kind() {
	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindDataLast, typeparser.TransformationKindCall:
		callNode, call := directCallForCallee(terminal.Callee())
		if call == nil {
			return nil
		}
		if terminal.Kind() == typeparser.TransformationKindCall {
			for callNode.Parent != nil && callNode.Parent.Kind == ast.KindParenthesizedExpression {
				callNode = callNode.Parent
			}
			if callNode.Parent == nil || callNode.Parent.Kind != ast.KindCallExpression || callNode.Parent.AsCallExpression().Expression != callNode {
				return nil
			}
			callNode = callNode.Parent
		}
		input := flow.TransformationInputNode(last)
		if input == nil || callNode.Flags&ast.NodeFlagsOptionalChain != 0 {
			return nil
		}
		return func(tracker *rewriter.Tracker) {
			tracker.ReplaceNode(sf, callNode, input, nil)
		}
	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
		callNode, call, index := containingCallArgumentForExit(terminal.Callee())
		if call == nil || call.Arguments == nil || index < 0 || callNode.Flags&ast.NodeFlagsOptionalChain != 0 {
			return nil
		}
		arguments := call.Arguments.Nodes
		minimum := 1
		if terminal.Kind() == typeparser.TransformationKindPipe {
			minimum = 2
		}
		if len(arguments) == minimum {
			var input *ast.Node
			if terminal.Kind() == typeparser.TransformationKindPipe {
				input = arguments[0]
			} else if call.Expression.Kind == ast.KindPropertyAccessExpression {
				input = call.Expression.AsPropertyAccessExpression().Expression
			}
			if input == nil {
				return nil
			}
			return func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(sf, callNode, input, nil)
			}
		}
		var span core.TextRange
		switch {
		case index < len(arguments)-1:
			span = core.NewTextRange(scanner.GetTokenPosOfNode(arguments[index], sf, false), scanner.GetTokenPosOfNode(arguments[index+1], sf, false))
		case index > 0:
			span = core.NewTextRange(arguments[index-1].End(), arguments[index].End())
			start := scanner.GetTokenPosOfNode(arguments[index], sf, false)
			trivia := sf.Text()[span.Pos():start]
			if strings.Contains(trivia, "//") || strings.Contains(trivia, "/*") {
				separator := scanner.SkipTrivia(sf.Text(), span.Pos())
				if separator >= start || sf.Text()[separator] != ',' {
					return nil
				}
				preserved := strings.TrimRight(sf.Text()[span.Pos():separator]+sf.Text()[separator+1:start], " \t")
				return func(tracker *rewriter.Tracker) {
					tracker.ReplaceTextRangeWithText(sf, span, preserved)
				}
			}
		default:
			return nil
		}
		return func(tracker *rewriter.Tracker) {
			tracker.DeleteRange(sf, span)
		}
	}
	return nil
}
