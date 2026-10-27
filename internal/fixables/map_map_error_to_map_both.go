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

var MapMapErrorToMapBothFix = fixable.Fixable{
	Name:        "mapMapErrorToMapBoth",
	Description: "Replace with Effect.mapBoth",
	ErrorCodes: []int32{
		tsdiag.Effect_0_Effect_1_transform_the_success_and_failure_channels_one_step_at_a_time_Effect_2_expresses_both_transformations_in_a_single_step_effect_mapMapErrorToMapBoth.Code(),
	},
	FixIDs: []string{"mapMapErrorToMapBoth_fix"},
	Run:    runMapMapErrorToMapBothFix,
}

func runMapMapErrorToMapBothFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMapMapErrorToMapBoth(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Fixable || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}
		target, receiver := mapBothReference(ctx, match)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(t *rewriter.Tracker) {
				var callee *ast.Node
				if receiver {
					callee = effectModuleMethod(t, match.SourceFile, target, match.ReplacementName)
				} else {
					callee = t.NewIdentifier(target.Text())
				}
				// Keep the callbacks in flow order so they are evaluated as before.
				firstName, secondName := "onSuccess", "onFailure"
				if match.FirstFailure {
					firstName, secondName = secondName, firstName
				}
				options := t.NewObjectLiteralExpression(t.NewNodeList([]*ast.Node{
					t.NewPropertyAssignment(nil, t.NewIdentifier(firstName), nil, nil, t.DeepCloneNode(match.FirstCallback)),
					t.NewPropertyAssignment(nil, t.NewIdentifier(secondName), nil, nil, t.DeepCloneNode(match.SecondCallback)),
				}), true)
				t.CollapsePipingFlowTransformations(match.SourceFile, match.Flow, match.Start, match.Start+1,
					rewriter.PipingFlowTransformationReplacement{Callee: callee, Arguments: t.NewNodeList([]*ast.Node{options})})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// mapBothReference finds a value that reaches the replacement API at the
// diagnostic: the Effect module receiver of one of the steps, or a binding in
// scope that is the replacement API itself (receiver false) or the Effect
// module (receiver true). It returns nil when the file has no such binding,
// for example when the steps only use named imports of map and mapError.
func mapBothReference(ctx *fixable.Context, match rules.MapMapErrorToMapBothMatch) (*ast.Node, bool) {
	method := match.ReplacementName
	for _, callee := range []*ast.Node{match.SecondCallee, match.FirstCallee} {
		if callee.Kind != ast.KindPropertyAccessExpression {
			continue
		}
		receiver := callee.AsPropertyAccessExpression().Expression
		if ctx.TypeParser.IsExpressionEffectModule(receiver) && effectModuleHasMethod(ctx, receiver, method) {
			return receiver, true
		}
	}
	location := match.SecondCallee
	symbols := ctx.Checker.GetSymbolsInScope(location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias)
	slices.SortFunc(symbols, func(left, right *ast.Symbol) int {
		return strings.Compare(ast.TSGetSymbolName(left), ast.TSGetSymbolName(right))
	})
	// Among module bindings, prefer the one the file imports Effect under.
	preferred := typeparser.FindEffectModuleIdentifier(match.SourceFile)
	var module *ast.Node
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
			if declaration.Parent == nil || declaration.Parent.Flags&ast.NodeFlagsConst == 0 || declaration.End() > location.Pos() {
				continue
			}
			reference = declaration.AsVariableDeclaration().Initializer
			if reference == nil || ctx.TypeParser.GetTypeAtLocation(name) != ctx.TypeParser.GetTypeAtLocation(reference) {
				continue
			}
		}
		// Prefer a direct binding to the replacement API over a module binding.
		if ctx.TypeParser.IsNodeReferenceToEffectModuleApi(reference, method) {
			return name, false
		}
		if (module == nil || name.Text() == preferred) && ctx.TypeParser.IsExpressionEffectModule(reference) && effectModuleHasMethod(ctx, name, method) {
			module = name
		}
	}
	if module != nil {
		return module, true
	}
	return nil, false
}

func effectModuleHasMethod(ctx *fixable.Context, receiver *ast.Node, method string) bool {
	property := ctx.Checker.GetPropertyOfType(ctx.TypeParser.GetTypeAtLocation(receiver), method)
	return property != nil && ast.TSGetSymbolValueDeclaration(property) != nil &&
		ctx.TypeParser.IsNodeReferenceToEffectModuleApi(ast.TSGetSymbolValueDeclaration(property).Name(), method)
}
