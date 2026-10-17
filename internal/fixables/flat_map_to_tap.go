package fixables

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
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
		target, receiver := tapReference(ctx, match.Callee)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.tap",
			Run: func(tracker *rewriter.Tracker) {
				if !tracker.RemoveTrailingPipingFlowTransformation(match.SourceFile, match.ReturnFlow) {
					return
				}
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
		return strings.Compare(ast.TSGetSymbolName(left), ast.TSGetSymbolName(right))
	})
	for _, symbol := range symbols {
		if len(ast.TSGetSymbolDeclarations(symbol)) != 1 || checker.Checker_isSymbolAssigned(ctx.Checker, symbol) {
			continue
		}
		declaration := ast.TSGetSymbolDeclarations(symbol)[0]
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
	return tap != nil && ast.TSGetSymbolValueDeclaration(tap) != nil && ctx.TypeParser.IsNodeReferenceToEffectModuleApi(ast.TSGetSymbolValueDeclaration(tap).Name(), "tap")
}
