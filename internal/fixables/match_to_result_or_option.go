package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MatchToResultOrOptionFix = fixable.Fixable{
	Name:        "matchToResultOrOption",
	Description: "Replace with Effect.result, Effect.option or Effect.either",
	ErrorCodes: []int32{
		tsdiag.Effect_0_reifies_the_outcome_directly_these_Effect_1_handlers_only_wrap_the_value_and_the_error_effect_matchToResultOrOption.Code(),
	},
	FixIDs: []string{"matchToResultOrOption_fix"},
	Run:    runMatchToResultOrOptionFix,
}

func runMatchToResultOrOptionFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMatchToResultOrOption(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Fixable || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}
		member := effectModuleMember(ctx, match.ReplacementName)
		location := match.Flow.Transformations().At(match.End).Callee()
		target := resolveModuleMember(ctx, location, typeparser.FindEffectModuleReference(match.SourceFile), member)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(t *rewriter.Tracker) {
				replacement := rewriter.PipingFlowTransformationReplacement{Callee: target.expression(t, member)}
				if match.Start == match.End {
					step := match.Flow.Transformations().At(match.Start)
					t.ReplacePipingFlowTransformation(match.SourceFile, &step, replacement)
					return
				}
				t.CollapsePipingFlowTransformations(match.SourceFile, match.Flow, match.Start, match.End, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
