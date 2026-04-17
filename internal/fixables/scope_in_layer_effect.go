package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var ScopeInLayerEffectScopedFix = fixable.Fixable{
	Name:        "scopeInLayerEffectScoped",
	Description: "Use scoped for Layer creation",
	ErrorCodes:  []int32{tsdiag.This_layer_construction_leaves_Scope_in_the_requirement_set_The_scoped_API_removes_Scope_from_the_resulting_requirements_effect_scopeInLayerEffect.Code()},
	FixIDs:      []string{"scopeInLayerEffect_scoped"},
	Run:         runScopeInLayerEffectScopedFix,
}

func runScopeInLayerEffectScopedFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	matches := rules.AnalyzeScopeInLayerEffect(ctx.TypeParser, c, ctx.SourceFile)
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
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(sf, match.MethodIdentifier, tracker.NewIdentifier("scoped"), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
