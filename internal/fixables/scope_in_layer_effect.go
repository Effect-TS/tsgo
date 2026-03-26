package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var ScopeInLayerEffectScopedFix = fixable.Fixable{
	Name:        "scopeInLayerEffectScoped",
	Description: "Use scoped for Layer creation",
	ErrorCodes:  []int32{tsdiag.Seems_like_you_are_constructing_a_layer_with_a_scope_in_the_requirements_Consider_using_scoped_instead_to_get_rid_of_the_scope_in_the_requirements_effect_scopeInLayerEffect.Code()},
	FixIDs:      []string{"scopeInLayerEffect_scoped"},
	Run:         runScopeInLayerEffectScopedFix,
}

func runScopeInLayerEffectScopedFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	matches := rules.AnalyzeScopeInLayerEffect(c, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		// Class declaration matches don't have a method identifier to replace
		if match.MethodIdentifier == nil {
			continue
		}

		sf := ctx.SourceFile

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Use scoped for Layer creation",
			Run: func(tracker *change.Tracker) {
				tracker.ReplaceNode(sf, match.MethodIdentifier, tracker.NewIdentifier("scoped"), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
