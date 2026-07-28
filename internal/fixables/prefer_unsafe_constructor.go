package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var PreferUnsafeConstructorFix = fixable.Fixable{
	Name:        "preferUnsafeConstructor",
	Description: "Replace Effect.runSync of a pure constructor with the synchronous *Unsafe variant",
	ErrorCodes: []int32{
		tsdiag.X_0_starts_a_fiber_to_run_a_pure_constructor_Use_the_synchronous_variant_1_instead_effect_preferUnsafeConstructor.Code(),
	},
	FixIDs: []string{"preferUnsafeConstructor_fix"},
	Run:    runPreferUnsafeConstructorFix,
}

func runPreferUnsafeConstructorFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile

	for _, match := range rules.AnalyzePreferUnsafeConstructor(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// A bare identifier callee (e.g. `import { make } from "effect/Scope"`) has no
		// local binding for the sibling, so renaming it would leave an unbound identifier.
		if match.InnerCallee.Kind != ast.KindPropertyAccessExpression {
			continue
		}

		m := match

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with " + m.UnsafeCalleeText,
			Run: func(tracker *rewriter.Tracker) {
				outerStart := scanner.GetTokenPosOfNode(m.OuterCall, sf, false)
				innerStart := scanner.GetTokenPosOfNode(m.InnerCall, sf, false)
				tracker.DeleteRange(sf, core.NewTextRange(outerStart, innerStart))
				tracker.ReplaceNode(sf, m.InnerCalleeName, tracker.NewIdentifier(m.UnsafeName), nil)
				tracker.DeleteRange(sf, core.NewTextRange(m.InnerCall.End(), m.OuterCall.End()))
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
