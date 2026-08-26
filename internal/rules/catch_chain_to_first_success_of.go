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

// CatchChainToFirstSuccessOf suggests firstSuccessOf for equivalent catch fallback chains.
var CatchChainToFirstSuccessOf = rule.Rule{
	Name:            "catchChainToFirstSuccessOf",
	Group:           "style",
	Description:     "Suggests Effect.firstSuccessOf for consecutive error-independent Effect.catch fallbacks when the error type is preserved",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Chained_Effect_catch_fallbacks_that_ignore_the_error_can_be_written_with_Effect_firstSuccessOf_Wrap_each_lazy_fallback_with_Effect_suspend_to_preserve_when_it_is_constructed_effect_catchChainToFirstSuccessOf.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchChainToFirstSuccessOf(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Chained_Effect_catch_fallbacks_that_ignore_the_error_can_be_written_with_Effect_firstSuccessOf_Wrap_each_lazy_fallback_with_Effect_suspend_to_preserve_when_it_is_constructed_effect_catchChainToFirstSuccessOf,
				nil,
			)
		}
		return diagnostics
	},
}

type CatchChainToFirstSuccessOfMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
}

type catchChainToFirstSuccessOfCandidate struct {
	transformation *typeparser.PipingFlowTransformation
	errorType      *checker.Type
}

// AnalyzeCatchChainToFirstSuccessOf finds consecutive zero-argument catch fallbacks.
func AnalyzeCatchChainToFirstSuccessOf(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []CatchChainToFirstSuccessOfMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []CatchChainToFirstSuccessOfMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		var chain []catchChainToFirstSuccessOfCandidate
		var initialError *checker.Type
		var outputError *checker.Type

		flush := func() {
			if catchChainPreservesFirstSuccessOfError(c, initialError, outputError, chain) {
				outermost := chain[len(chain)-1].transformation
				matches = append(matches, CatchChainToFirstSuccessOfMatch{
					SourceFile: sf,
					Location:   scanner.GetErrorRangeForNode(sf, outermost.Callee),
				})
			}
			chain = nil
			initialError = nil
			outputError = nil
		}

		for i := range flow.Transformations {
			transformation := &flow.Transformations[i]
			candidate, ok := analyzeCatchChainToFirstSuccessOfCandidate(tp, c, transformation)
			if !ok {
				flush()
				continue
			}

			if len(chain) == 0 {
				inputType := flow.Subject.OutType
				if i > 0 {
					inputType = flow.Transformations[i-1].OutType
				}
				inputEffect := tp.StrictEffectType(inputType, transformation.Callee)
				if inputEffect == nil || inputEffect.E == nil {
					flush()
					continue
				}
				initialError = inputEffect.E
			}

			outputEffect := tp.StrictEffectType(transformation.OutType, transformation.Callee)
			if outputEffect == nil || outputEffect.E == nil {
				flush()
				continue
			}
			chain = append(chain, candidate)
			outputError = outputEffect.E
		}

		flush()
	}

	return matches
}

func analyzeCatchChainToFirstSuccessOfCandidate(tp *typeparser.TypeParser, c *checker.Checker, transformation *typeparser.PipingFlowTransformation) (catchChainToFirstSuccessOfCandidate, bool) {
	if tp == nil || c == nil || transformation == nil || transformation.Callee == nil ||
		!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "catch") || len(transformation.Args) != 1 {
		return catchChainToFirstSuccessOfCandidate{}, false
	}

	lazy := typeparser.ParseLazyExpression(transformation.Args[0], true)
	if lazy == nil || lazy.Expression == nil {
		return catchChainToFirstSuccessOfCandidate{}, false
	}

	handlerType := tp.GetTypeAtLocation(transformation.Args[0])
	if handlerType == nil {
		return catchChainToFirstSuccessOfCandidate{}, false
	}
	signatures := c.GetSignaturesOfType(handlerType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		return catchChainToFirstSuccessOfCandidate{}, false
	}

	fallbackType := tp.StrictEffectType(c.GetReturnTypeOfSignature(signatures[0]), lazy.Expression)
	if fallbackType == nil || fallbackType.E == nil {
		return catchChainToFirstSuccessOfCandidate{}, false
	}

	return catchChainToFirstSuccessOfCandidate{
		transformation: transformation,
		errorType:      fallbackType.E,
	}, true
}

func catchChainPreservesFirstSuccessOfError(c *checker.Checker, initialError *checker.Type, outputError *checker.Type, chain []catchChainToFirstSuccessOfCandidate) bool {
	if c == nil || initialError == nil || outputError == nil || len(chain) < 2 ||
		initialError.Flags()&checker.TypeFlagsAny != 0 || outputError.Flags()&checker.TypeFlagsAny != 0 {
		return false
	}

	finalError := chain[len(chain)-1].errorType
	// catch retains only the final fallback error, while firstSuccessOf unions all
	// candidate errors. The union is unchanged only when each earlier error fits.
	if finalError == nil || finalError.Flags()&checker.TypeFlagsAny != 0 ||
		!c.IsTypeAssignableTo(finalError, outputError) || !c.IsTypeAssignableTo(outputError, finalError) ||
		!c.IsTypeAssignableTo(initialError, outputError) {
		return false
	}

	for _, candidate := range chain[:len(chain)-1] {
		if candidate.errorType == nil || candidate.errorType.Flags()&checker.TypeFlagsAny != 0 ||
			!c.IsTypeAssignableTo(candidate.errorType, outputError) {
			return false
		}
	}
	return true
}
