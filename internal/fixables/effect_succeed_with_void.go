package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
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

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeEffectSucceedWithVoid(ctx.TypeParser, c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.void",
			Run: func(tracker *rewriter.Tracker) {
				replacementNode := effectModuleMethod(tracker, sf, match.EffectModuleNode, "void")
				ast.SetParentInChildren(replacementNode)
				tracker.ReplaceNode(sf, match.CallNode, replacementNode, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		continue
	}

	return nil
}
