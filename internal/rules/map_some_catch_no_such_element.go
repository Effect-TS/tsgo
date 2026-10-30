package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var MapSomeCatchNoSuchElement = rule.Rule{
	Name:            "mapSomeCatchNoSuchElement",
	Group:           "style",
	Description:     "Suggests Effect.catchNoSuchElement instead of wrapping successes in Option.some and catching NoSuchElementError into Option.none",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Use_Effect_catchNoSuchElement_instead_of_wrapping_successes_in_Option_some_and_catching_NoSuchElementError_into_Option_none_effect_mapSomeCatchNoSuchElement.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diagnostics []*ast.Diagnostic
		for _, match := range AnalyzeMapSomeCatchNoSuchElement(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
			diagnostics = append(diagnostics, ctx.NewDiagnostic(ctx.SourceFile, match.Location, tsdiag.Use_Effect_catchNoSuchElement_instead_of_wrapping_successes_in_Option_some_and_catching_NoSuchElementError_into_Option_none_effect_mapSomeCatchNoSuchElement, nil))
		}
		return diagnostics
	},
}

type MapSomeCatchNoSuchElementMatch struct {
	Flow       *typeparser.PipingFlow
	Start, End int
	Location   core.TextRange
}

// noSuchElementErrorTypeId is the brand key of Cause.NoSuchElementError, the
// property Effect.catchNoSuchElement checks at runtime.
const noSuchElementErrorTypeId = "~effect/Cause/NoSuchElementError"

func AnalyzeMapSomeCatchNoSuchElement(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MapSomeCatchNoSuchElementMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []MapSomeCatchNoSuchElementMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(
			func(step typeparser.PipingFlowTransformation) bool { return mapSomeCatchWrapsSome(tp, &step) },
			func(step typeparser.PipingFlowTransformation) bool {
				return mapSomeCatchCatchesNoSuchElement(tp, &step)
			},
		).All() {
			start := sequence.Start()
			wrap, catch := sequence.Transformations().At(0), sequence.Transformations().At(1)
			if hasExplicitTypeArguments(&wrap) || hasExplicitTypeArguments(&catch) {
				continue
			}
			handler := ast.SkipParentheses(catch.Args().At(1))
			if !catchHandlerIgnoresError(tp, c, handler) || !catchHandlerReturnsSucceedNone(tp, handler) {
				continue
			}
			input := tp.StrictEffectType(flow.TransformationInputType(start))
			if input == nil || !noSuchElementErrorCatchable(tp, input.E()) {
				continue
			}
			output := tp.StrictEffectType(catch.OutType())
			if output == nil || !mapSomeCatchMatchesCatchNoSuchElement(tp, c, input, output) {
				continue
			}
			matches = append(matches, MapSomeCatchNoSuchElementMatch{Flow: flow, Start: start, End: start + 1, Location: scanner.GetErrorRangeForNode(sf, catch.Callee())})
		}
	}
	return matches
}

func mapSomeCatchWrapsSome(tp *typeparser.TypeParser, step *typeparser.PipingFlowTransformation) bool {
	return step.Args().Len() == 0 && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "asSome") ||
		step.Args().Len() == 1 && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "map") && isOptionSomeMapper(tp, step.Args().At(0))
}

func mapSomeCatchCatchesNoSuchElement(tp *typeparser.TypeParser, step *typeparser.PipingFlowTransformation) bool {
	if step.Args().Len() != 2 {
		return false
	}
	tag := ast.SkipParentheses(step.Args().At(0))
	return tag.Kind == ast.KindStringLiteral && tag.AsStringLiteral().Text == "NoSuchElementError" &&
		tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "catchTag")
}

// The fix replaces the pair's result type with catchNoSuchElement's
// Effect<Option<A>, Exclude<E, NoSuchElementError>, R>. Callback annotations can
// change any channel of the pair, so compare the pair's output with that shape.
// Assignability accepts any in place of every type, so use type identity.
func mapSomeCatchMatchesCatchNoSuchElement(tp *typeparser.TypeParser, c *checker.Checker, input, output *typeparser.Effect) bool {
	identical := func(source, target *checker.Type) bool {
		return checker.Checker_isTypeIdenticalTo(c, source, target)
	}
	for _, member := range tp.UnrollUnionMembers(output.A()) {
		tag := tp.GetTypeOfPropertyByName(member, "_tag")
		if tag == nil || tag.Flags()&checker.TypeFlagsStringLiteral == 0 {
			return false
		}
		if text, ok := tag.AsLiteralType().Value().(string); !ok || text != "Some" && text != "None" {
			return false
		}
		// Option.none() and Effect.succeedNone contribute Option<never>.
		arguments := c.GetTypeArguments(member)
		if len(arguments) != 1 || arguments[0].Flags()&checker.TypeFlagsNever == 0 && !identical(arguments[0], input.A()) {
			return false
		}
	}
	var remaining []*checker.Type
	for _, member := range tp.UnrollUnionMembers(input.E()) {
		if tp.GetTypeOfPropertyByName(member, noSuchElementErrorTypeId) == nil {
			remaining = append(remaining, member)
		}
	}
	var produced []*checker.Type
	for _, member := range tp.UnrollUnionMembers(output.E()) {
		if member.Flags()&checker.TypeFlagsNever == 0 {
			produced = append(produced, member)
		}
	}
	contains := func(types []*checker.Type, target *checker.Type) bool {
		return slices.ContainsFunc(types, func(candidate *checker.Type) bool { return identical(candidate, target) })
	}
	for _, member := range produced {
		if !contains(remaining, member) {
			return false
		}
	}
	for _, member := range remaining {
		if !contains(produced, member) {
			return false
		}
	}
	return identical(output.R(), input.R())
}

// catchTag matches by _tag while catchNoSuchElement checks the brand, so every
// member that may carry the tag must be the real Cause.NoSuchElementError.
// Untagged, broadly tagged or generic members fail closed.
func noSuchElementErrorCatchable(tp *typeparser.TypeParser, errorType *checker.Type) bool {
	if errorType == nil || errorType.Flags()&checker.TypeFlagsNever != 0 {
		return false
	}
	found := false
	for _, member := range tp.UnrollUnionMembers(errorType) {
		branded := tp.GetTypeOfPropertyByName(member, noSuchElementErrorTypeId) != nil
		found = found || branded
		tag := tp.GetTypeOfPropertyByName(member, "_tag")
		if tag == nil {
			return false
		}
		for _, value := range tp.UnrollUnionMembers(tag) {
			if value.Flags()&checker.TypeFlagsStringLiteral == 0 {
				return false
			}
			if text, ok := value.AsLiteralType().Value().(string); !ok || text == "NoSuchElementError" && !branded {
				return false
			}
		}
	}
	return found
}
