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
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// The fix removes one callee and replaces the other, so both must be
		// entity names whose evaluation runs no code.
		if !ast.IsEntityNameExpression(match.First.Callee()) || !ast.IsEntityNameExpression(match.Second.Callee()) ||
			rules.HasExplicitTypeArguments(&match.Second) || mapMapErrorPathHasTypeArguments(match.First, match.Second) ||
			!mapMapErrorKeepsEvaluationOrder(match.First, match.Second, match.Eager) {
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

// mapMapErrorPathHasTypeArguments reports explicit type arguments on any call
// from the first step up to the call that applies the second step: the first
// step itself, its curried application or containing pipe, empty pipe calls in
// between, and the second step's application or containing pipe. Removing the
// first step changes the input type of every one of them, so their type
// arguments would no longer match. It also reports true when the second step's
// application is not an ancestor of the first step.
func mapMapErrorPathHasTypeArguments(first, second typeparser.PipingFlowTransformation) bool {
	application := mapMapErrorStepApplication(second)
	for node := first.Callee(); node != nil; node = node.Parent {
		if node.Kind == ast.KindCallExpression {
			typeArguments := node.AsCallExpression().TypeArguments
			if typeArguments != nil && len(typeArguments.Nodes) > 0 {
				return true
			}
		}
		if node == application {
			return false
		}
	}
	return true
}

// mapMapErrorStepApplication returns the call that applies a step to its input.
func mapMapErrorStepApplication(step typeparser.PipingFlowTransformation) *ast.Node {
	call := step.Callee().Parent
	if call == nil || (step.Kind() != typeparser.TransformationKindCall && !rules.IsPipingFlowArgumentKind(step.Kind())) {
		return call
	}
	// A curried step is applied by the call around its factory, and a pipe
	// argument by the pipe or Effect.fn call that holds it.
	application := call.Parent
	for application != nil && application.Kind == ast.KindParenthesizedExpression {
		application = application.Parent
	}
	return application
}

// mapMapErrorKeepsEvaluationOrder reports whether the fix keeps the observable
// order of evaluation. Steps in one argument list keep it: their callbacks are
// evaluated before any of them runs. Otherwise the fix moves the first callback
// into the second step and the first step's work into the merged step. Only a
// function literal can move across other code, because evaluating it runs none.
func mapMapErrorKeepsEvaluationOrder(first, second typeparser.PipingFlowTransformation, eager bool) bool {
	if mapMapErrorShareArgumentList(first, second) {
		return true
	}
	if !isFunctionLiteral(first.Args().At(0)) && !mapMapErrorFirstCallbackMovesFreely(first, second) {
		return false
	}
	// A non-eager step only builds an effect, so moving its work is unobservable.
	return !eager || mapMapErrorFirstWorkMovesFreely(second)
}

// mapMapErrorFirstCallbackMovesFreely reports whether nothing observable runs
// between the first callback's evaluation and the second step's callback
// position, apart from applying the first step. That holds when the first step
// evaluates its input before its callback and is the first step of its pipe,
// and the second step evaluates its input before its callback. A curried step
// evaluates its callback before its input, and earlier steps of the first pipe
// run before the second pipe's arguments are evaluated.
func mapMapErrorFirstCallbackMovesFreely(first, second typeparser.PipingFlowTransformation) bool {
	if second.Kind() != typeparser.TransformationKindDataFirst && !rules.IsPipingFlowArgumentKind(second.Kind()) {
		return false
	}
	if first.Kind() == typeparser.TransformationKindDataFirst {
		return true
	}
	if !rules.IsPipingFlowArgumentKind(first.Kind()) {
		return false
	}
	index, _ := mapMapErrorArgumentIndex(first)
	if first.Kind() == typeparser.TransformationKindPipeable {
		return index == 0
	}
	// Function.pipe and Effect.fn hold their subject or body before the steps.
	return index == 1
}

// mapMapErrorFirstWorkMovesFreely reports whether an eager first step can run
// in the merged step without crossing observable code. When the first step is
// applied before the second callback is evaluated, the merged step runs that
// work after the second callback and, for a pipe argument, after the arguments
// that follow it.
func mapMapErrorFirstWorkMovesFreely(second typeparser.PipingFlowTransformation) bool {
	switch {
	case second.Kind() == typeparser.TransformationKindCall || second.Kind() == typeparser.TransformationKindDataLast:
		return true
	case second.Kind() == typeparser.TransformationKindDataFirst:
		return isFunctionLiteral(second.Args().At(0))
	case rules.IsPipingFlowArgumentKind(second.Kind()):
		index, count := mapMapErrorArgumentIndex(second)
		return isFunctionLiteral(second.Args().At(0)) && index == count-1
	}
	return false
}

// mapMapErrorArgumentIndex returns the position of a pipe argument step in its
// call's argument list and the list's length, or -1 when it is not found.
func mapMapErrorArgumentIndex(step typeparser.PipingFlowTransformation) (int, int) {
	stepCall := step.Callee().Parent
	if stepCall == nil || stepCall.Parent == nil || stepCall.Parent.Kind != ast.KindCallExpression {
		return -1, 0
	}
	arguments := stepCall.Parent.AsCallExpression().Arguments
	if arguments == nil {
		return -1, 0
	}
	return slices.Index(arguments.Nodes, stepCall), len(arguments.Nodes)
}

func isFunctionLiteral(node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	return node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression
}

func mapMapErrorShareArgumentList(first, second typeparser.PipingFlowTransformation) bool {
	if !rules.IsPipingFlowArgumentKind(first.Kind()) || !rules.IsPipingFlowArgumentKind(second.Kind()) {
		return false
	}
	firstStep, secondStep := first.Callee().Parent, second.Callee().Parent
	return firstStep != nil && secondStep != nil && firstStep.Parent != nil && firstStep.Parent == secondStep.Parent
}
