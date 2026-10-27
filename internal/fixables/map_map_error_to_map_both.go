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
		member := effectModuleMember(ctx, match.ReplacementName)
		target := mapBothReference(ctx, match, member)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(t *rewriter.Tracker) {
				callee := target.expression(t, member)
				// Keep the callbacks in flow order so they are evaluated as before.
				firstName, secondName := "onSuccess", "onFailure"
				if match.FirstFailure {
					firstName, secondName = secondName, firstName
				}
				options := t.NewObjectLiteralExpression(t.NewNodeList([]*ast.Node{
					t.NewPropertyAssignment(nil, t.NewIdentifier(firstName), nil, nil, t.DeepCloneNode(match.First.Args().At(0))),
					t.NewPropertyAssignment(nil, t.NewIdentifier(secondName), nil, nil, t.DeepCloneNode(match.Second.Args().At(0))),
				}), true)
				// The first step is the second step's input, so removing it never
				// overlaps the second step's callee or callback.
				if !removePipingFlowTransformation(t, match.SourceFile, &match.First) {
					return
				}
				t.ReplaceNode(match.SourceFile, match.Second.Callee(), callee, nil)
				t.ReplaceNode(match.SourceFile, match.Second.Args().At(0), options, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// mapBothReference finds a reference that reaches the replacement API at the
// diagnostic: the file's Effect import, or a binding in scope that is the
// replacement API itself or the Effect module. It returns nil when the file
// has no such binding, for example when the steps only use named imports of
// map and mapError.
func mapBothReference(ctx *fixable.Context, match rules.MapMapErrorToMapBothMatch, member moduleMember) *moduleMemberReference {
	location := match.Second.Callee()
	if reference := importedModuleMember(ctx, location, typeparser.FindEffectModuleReference(match.SourceFile), member); reference != nil {
		return reference
	}
	symbols := ctx.Checker.GetSymbolsInScope(location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias)
	slices.SortFunc(symbols, func(left, right *ast.Symbol) int {
		return strings.Compare(ast.TSGetSymbolName(left), ast.TSGetSymbolName(right))
	})
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
		if member.isMember(reference) {
			return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}}
		}
		if module == nil && member.isModule(reference) && typeHasMember(ctx, ctx.TypeParser.GetTypeAtLocation(name), member) {
			module = name
		}
	}
	if module != nil {
		return &moduleMemberReference{path: typeparser.ModuleReference{module.Text()}, viaModule: true}
	}
	return nil
}
