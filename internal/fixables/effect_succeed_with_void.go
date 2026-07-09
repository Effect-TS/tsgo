package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/typescript-go/shim/scanner"
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

		// Extract the Effect module name, preserving the import alias
		effectModuleName := "Effect"
		if match.EffectModuleNode != nil && match.EffectModuleNode.Kind == ast.KindIdentifier {
			effectModuleName = scanner.GetTextOfNode(match.EffectModuleNode)
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.void",
			Run: func(tracker *rewriter.Tracker) {
				// Build Effect.void as a PropertyAccessExpression
				effectModuleId := tracker.NewIdentifier(effectModuleName)
				replacementNode := tracker.NewPropertyAccessExpression(effectModuleId, nil, tracker.NewIdentifier("void"), ast.NodeFlagsNone)
				ast.SetParentInChildren(replacementNode)
				tracker.ReplaceNode(sf, match.CallNode, replacementNode, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
