package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var EffectMapVoidFix = fixable.Fixable{
	Name:        "effectMapVoid",
	Description: "Replace with Effect.asVoid",
	ErrorCodes:  []int32{tsdiag.Effect_asVoid_can_be_used_instead_to_discard_the_success_value_effect_effectMapVoid.Code()},
	FixIDs:      []string{"effectMapVoid_fix"},
	Run:         runEffectMapVoidFix,
}

func runEffectMapVoidFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeEffectMapVoid(c, sf)
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
			Description: "Replace with Effect.asVoid",
			Run: func(tracker *change.Tracker) {
				// Build Effect.asVoid as a PropertyAccessExpression
				effectModuleId := tracker.NewIdentifier(effectModuleName)
				replacementNode := tracker.NewPropertyAccessExpression(effectModuleId, nil, tracker.NewIdentifier("asVoid"), ast.NodeFlagsNone)
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
