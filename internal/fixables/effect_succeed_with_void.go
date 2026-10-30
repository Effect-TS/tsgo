package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var EffectSucceedWithVoidFix = fixable.Fixable{
	Name:        "effectSucceedWithVoid",
	Description: "Replace with Effect.void",
	ErrorCodes:  []int32{tsdiag.Effect_void_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithVoid.Code()},
	FixIDs:      []string{"effectSucceedWithVoid_fix"},
	Run:         runEffectSucceedWithVoidFix,
}

func runEffectSucceedWithVoidFix(ctx *fixable.Context) []ls.CodeAction {
	return runEffectSucceedConstantFix(ctx, rules.AnalyzeEffectSucceedWithVoid(ctx.TypeParser, ctx.Checker, ctx.SourceFile), "void")
}

// runEffectSucceedConstantFix replaces the constant subject and the Effect.succeed step of a
// match with Effect.<memberName>, keeping any later steps. The fix is offered only when the
// step can be dropped without losing a side effect and a reference to the constant resolves
// at the call site.
func runEffectSucceedConstantFix(ctx *fixable.Context, matches []rules.EffectSucceedConstantMatch, memberName string) []ls.CodeAction {
	sf := ctx.SourceFile
	member := effectModuleMember(ctx, memberName)
	for _, match := range matches {
		diagRange := match.Location
		if !match.Rewritable || (!diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange)) {
			continue
		}
		reference := resolveModuleMember(ctx, match.Flow.Node(), typeparser.FindEffectModuleReference(sf), member)
		if reference == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + memberName,
			Run: func(tracker *rewriter.Tracker) {
				replacementNode := reference.expression(tracker, member)
				ast.SetParentInChildren(replacementNode)
				tracker.ReplacePipingFlowPrefix(sf, match.Flow, 1, replacementNode)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
