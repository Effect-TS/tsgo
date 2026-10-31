package rules

import (
	"sort"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// DiscardedRunPromiseToRunFork reports runPromise runs whose Promise is
// discarded: any failure or defect of the effect then becomes an unhandled
// rejection, while runFork states the fire-and-forget intent with a Fiber.
var DiscardedRunPromiseToRunFork = rule.Rule{
	Name:            "discardedRunPromiseToRunFork",
	Group:           "correctness",
	Description:     "Detects discarded Promises from Effect runPromise runners that should use runFork",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.The_Promise_returned_by_0_is_discarded_so_any_failure_or_defect_becomes_an_unhandled_rejection_Use_1_to_run_it_fire_and_forget_as_a_Fiber_effect_discardedRunPromiseToRunFork.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeDiscardedRunPromiseToRunFork(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.The_Promise_returned_by_0_is_discarded_so_any_failure_or_defect_becomes_an_unhandled_rejection_Use_1_to_run_it_fire_and_forget_as_a_Fiber_effect_discardedRunPromiseToRunFork,
				nil,
				match.RunnerName,
				match.ReplacementName,
			)
		}
		return diagnostics
	},
}

// DiscardedRunPromiseToRunForkMatch holds what the diagnostic and quick fix
// need. RenameNode is the runner's property name to rename to RenameTo, or nil
// when the fix is withheld.
type DiscardedRunPromiseToRunForkMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	RunnerName      string
	ReplacementName string
	RenameNode      *ast.Node
	RenameTo        string
}

// AnalyzeDiscardedRunPromiseToRunFork finds piping flows ending in a
// runPromise run whose resulting Promise is discarded. A runner that is not
// the flow's last step is consumed by the next step.
func AnalyzeDiscardedRunPromiseToRunFork(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []DiscardedRunPromiseToRunForkMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []DiscardedRunPromiseToRunForkMatch
	for _, flow := range tp.PipingFlows(sf, false).All() {
		transformations := flow.Transformations()
		if transformations.Len() == 0 {
			continue
		}
		last := transformations.At(transformations.Len() - 1)
		switch last.Kind() {
		case typeparser.TransformationKindCall, typeparser.TransformationKindCallWithOptionals, typeparser.TransformationKindDataLast:
		default:
			continue
		}
		if last.Callee() == nil || !isDiscardedExpression(flow.Node()) {
			continue
		}
		match, locationNode, ok := discardedRunPromiseRunner(tp, flow.Node(), last)
		if !ok {
			continue
		}
		if match.RenameNode != nil && last.Kind() == typeparser.TransformationKindCallWithOptionals &&
			tp.SupportedEffectVersion() == typeparser.EffectMajorV3 {
			// Effect v3 runFork options do not accept runPromise's signal.
			match.RenameNode = nil
		}
		match.SourceFile = sf
		match.Location = scanner.GetErrorRangeForNode(sf, locationNode)
		matches = append(matches, match)
	}

	sort.Slice(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})
	return matches
}

// discardedRunner describes a recognized Promise runner and the Fiber runner
// its fix renames it to.
type discardedRunner struct {
	name        string
	replacement string
	renameTo    string
	// ownsReplacement reports whether the runner's receiver is known to expose
	// the replacement with its declared signature, so the rename is sound.
	ownsReplacement func(tp *typeparser.TypeParser, receiver *ast.Node) bool
}

var (
	effectRunPromiseRunner = discardedRunner{
		name: "Effect.runPromise", replacement: "Effect.runFork", renameTo: "runFork",
		ownsReplacement: (*typeparser.TypeParser).IsExpressionEffectModule,
	}
	effectRunPromiseWithRunner = discardedRunner{
		name: "Effect.runPromiseWith", replacement: "Effect.runForkWith", renameTo: "runForkWith",
		ownsReplacement: (*typeparser.TypeParser).IsExpressionEffectModule,
	}
	runtimeRunPromiseRunner = discardedRunner{
		name: "Runtime.runPromise", replacement: "Runtime.runFork", renameTo: "runFork",
		ownsReplacement: (*typeparser.TypeParser).IsExpressionEffectRuntimeModule,
	}
	managedRuntimeRunPromiseRunner = discardedRunner{
		name: "runPromise", replacement: "runFork", renameTo: "runFork",
		ownsReplacement: (*typeparser.TypeParser).IsExpressionPlainEffectManagedRuntime,
	}
)

// discardedRunPromiseRunner classifies the runner of the last flow step and
// returns the node the diagnostic is reported on.
func discardedRunPromiseRunner(tp *typeparser.TypeParser, call *ast.Node, transformation typeparser.PipingFlowTransformation) (DiscardedRunPromiseToRunForkMatch, *ast.Node, bool) {
	callee := ast.SkipParentheses(transformation.Callee())
	newMatch := func(runnerNode *ast.Node, runner discardedRunner, renameable bool) (DiscardedRunPromiseToRunForkMatch, *ast.Node, bool) {
		match := DiscardedRunPromiseToRunForkMatch{
			RunnerName:      runner.name,
			ReplacementName: runner.replacement,
			RenameTo:        runner.renameTo,
		}
		// Renaming in place is only sound on the receiver that owns both
		// runners: the Effect or Runtime module itself, or a plain
		// ManagedRuntime. A narrowed or synthesized receiver may lack the
		// replacement or change its type, so it keeps only the diagnostic.
		if renameable && runnerNode.Kind == ast.KindPropertyAccessExpression {
			access := runnerNode.AsPropertyAccessExpression()
			if runner.ownsReplacement(tp, ast.SkipParentheses(access.Expression)) {
				match.RenameNode = access.Name()
			}
		}
		return match, runnerNode, true
	}

	kind := transformation.Kind()
	isApplied := kind == typeparser.TransformationKindCall || kind == typeparser.TransformationKindCallWithOptionals
	switch {
	case isApplied && tp.IsNodeReferenceToEffectModuleApi(callee, "runPromise"):
		return newMatch(callee, effectRunPromiseRunner, true)
	case isApplied && tp.IsNodeReferenceToEffectManagedRuntimeMember(callee, "runPromise"):
		return newMatch(callee, managedRuntimeRunPromiseRunner, true)
	case kind == typeparser.TransformationKindDataLast && tp.IsNodeReferenceToEffectRuntimeModuleApi(callee, "runPromise"):
		return newMatch(callee, runtimeRunPromiseRunner, true)
	case !isApplied:
		return DiscardedRunPromiseToRunForkMatch{}, nil, false
	}

	// Curried: the step's callee is the factory call, as in
	// Effect.runPromiseWith(context)(effect).
	if callee.Kind == ast.KindCallExpression {
		if factory := callee.AsCallExpression(); factory.Arguments != nil && len(factory.Arguments.Nodes) == 1 {
			runnerNode := ast.SkipParentheses(factory.Expression)
			if runner, ok := discardedRunPromiseFactory(tp, runnerNode); ok {
				return newMatch(runnerNode, runner, true)
			}
		}
	}

	// Hoisted: the callee is a binding of the factory call's result, so there
	// is no runner name to rename in place.
	if owner := tp.ReturnedFunctionCallOwner(call); owner != nil {
		if runner, ok := discardedRunPromiseFactory(tp, owner); ok {
			return newMatch(callee, runner, false)
		}
	}
	return DiscardedRunPromiseToRunForkMatch{}, nil, false
}

// discardedRunPromiseFactory recognizes the runners that take a context or
// runtime first and return the Promise runner.
func discardedRunPromiseFactory(tp *typeparser.TypeParser, node *ast.Node) (discardedRunner, bool) {
	switch {
	case tp.IsNodeReferenceToEffectModuleApi(node, "runPromiseWith"):
		return effectRunPromiseWithRunner, true
	case tp.IsNodeReferenceToEffectRuntimeModuleApi(node, "runPromise"):
		return runtimeRunPromiseRunner, true
	}
	return discardedRunner{}, false
}

// isDiscardedExpression reports whether the value of node is discarded: it is
// an expression statement, the operand of void, or a comma operand whose value
// is not used. Parentheses are transparent.
func isDiscardedExpression(node *ast.Node) bool {
	for node != nil && node.Parent != nil {
		parent := ast.WalkUpParenthesizedExpressions(node.Parent)
		if parent == nil {
			return false
		}
		switch parent.Kind {
		case ast.KindExpressionStatement, ast.KindVoidExpression:
			return true
		case ast.KindBinaryExpression:
			binary := parent.AsBinaryExpression()
			if binary.OperatorToken == nil || binary.OperatorToken.Kind != ast.KindCommaToken {
				return false
			}
			if ast.SkipParentheses(binary.Left) == node {
				return true
			}
			node = parent
			continue
		}
		return false
	}
	return false
}
