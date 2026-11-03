package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MatchBooleanToIsSuccessFix = fixable.Fixable{
	Name:        "matchBooleanToIsSuccess",
	Description: "Replace Effect.match boolean folds with Effect.isSuccess or Effect.isFailure",
	ErrorCodes: []int32{
		tsdiag.Effect_isSuccess_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess.Code(),
		tsdiag.Effect_isFailure_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess.Code(),
	},
	FixIDs: []string{"matchBooleanToIsSuccess_fix"},
	Run:    runMatchBooleanToIsSuccessFix,
}

func runMatchBooleanToIsSuccessFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMatchBooleanToIsSuccess(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		member := effectModuleMember(ctx, match.ReplacementName)
		reference, namedImports := calleeModuleMemberReference(ctx, match.Callee, typeparser.FindEffectModuleReference(ctx.SourceFile), member)
		if reference == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				transformation := match.Flow.Transformations().At(match.Index)
				replaced := tracker.ReplacePipingFlowTransformation(match.SourceFile, &transformation,
					rewriter.PipingFlowTransformationReplacement{Callee: reference.expression(tracker, member)})
				if replaced && namedImports != nil {
					elements := namedImports.AsNamedImports().Elements
					tracker.InsertImportSpecifierAtIndex(match.SourceFile,
						tracker.NewImportSpecifier(false, nil, tracker.NewIdentifier(member.name)), namedImports, len(elements.Nodes))
				}
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
