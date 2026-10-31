package rules

import (
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
		for _, match := range AnalyzeMapSomeCatchNoSuchElement(ctx.TypeParser, ctx.SourceFile) {
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

func AnalyzeMapSomeCatchNoSuchElement(tp *typeparser.TypeParser, sf *ast.SourceFile) []MapSomeCatchNoSuchElementMatch {
	if tp == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
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
			if !mapSomeCatchHandlerIsPlain(catch.Args().At(1)) || !catchHandlerReturnsSucceedNone(tp, catch.Args().At(1)) {
				continue
			}
			input := tp.StrictEffectType(flow.TransformationInputType(start))
			if input == nil || !noSuchElementErrorCatchable(tp, input.E()) {
				continue
			}
			matches = append(matches, MapSomeCatchNoSuchElementMatch{Flow: flow, Start: start, End: start + 1, Location: scanner.GetErrorRangeForNode(sf, catch.Callee())})
		}
	}
	return matches
}

// Return type annotations can change any channel of the pair's result, which
// Effect.catchNoSuchElement would replace with Effect<Option<A>, Exclude<E, NoSuchElementError>, R>.
func mapSomeCatchWrapsSome(tp *typeparser.TypeParser, step *typeparser.PipingFlowTransformation) bool {
	if step.Args().Len() == 0 {
		return tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "asSome")
	}
	if step.Args().Len() != 1 || !tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "map") || !isOptionSomeMapper(tp, step.Args().At(0)) {
		return false
	}
	lazy := typeparser.ParseLazyExpression(step.Args().At(0), typeparser.LazyExpressionNone)
	return lazy == nil || lazy.Node().Type() == nil
}

func mapSomeCatchCatchesNoSuchElement(tp *typeparser.TypeParser, step *typeparser.PipingFlowTransformation) bool {
	if step.Args().Len() != 2 {
		return false
	}
	tag := ast.SkipParentheses(step.Args().At(0))
	return tag.Kind == ast.KindStringLiteral && tag.AsStringLiteral().Text == "NoSuchElementError" &&
		tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "catchTag")
}

// The handler's returned flow is checked to be exactly a successful None, so
// it cannot use the error; destructuring would still read it. A return type
// annotation could change the pair's result type.
func mapSomeCatchHandlerIsPlain(handler *ast.Node) bool {
	lazy := typeparser.ParseLazyExpression(handler, typeparser.LazyExpressionNone)
	if lazy == nil || lazy.Node().Type() != nil {
		return false
	}
	for _, param := range lazy.Params().All() {
		if param.Name().Kind != ast.KindIdentifier {
			return false
		}
	}
	return true
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
