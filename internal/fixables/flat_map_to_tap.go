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
		tap := effectModuleMember(ctx, "tap")
		target := tapReference(ctx, match.SourceFile, match.Callee, tap)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.tap",
			Run: func(tracker *rewriter.Tracker) {
				if !tracker.RemoveTrailingPipingFlowTransformation(match.SourceFile, match.ReturnFlow) {
					return
				}
				tracker.ReplaceNode(match.SourceFile, match.Callee, target.expression(tracker, tap), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// tapReference finds a reference that reaches Effect.tap at callee: the file's
// Effect import, or a binding in scope that is tap itself or the Effect module.
func tapReference(ctx *fixable.Context, sf *ast.SourceFile, callee *ast.Node, tap moduleMember) *moduleMemberReference {
	if reference := importedModuleMember(ctx, callee, typeparser.FindEffectModuleReference(sf), tap); reference != nil {
		return reference
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
		if tap.isMember(reference) {
			return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}}
		}
		if tap.isModule(reference) && typeHasMember(ctx, ctx.TypeParser.GetTypeAtLocation(name), tap) {
			return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}, viaModule: true}
		}
	}
	return nil
}
