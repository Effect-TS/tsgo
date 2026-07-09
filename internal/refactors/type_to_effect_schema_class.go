package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/schemagen"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var TypeToEffectSchemaClass = refactor.Refactor{
	Name:        "typeToEffectSchemaClass",
	Description: "Generate Schema.Class from type",
	Kind:        "rewrite.effect.typeToEffectSchemaClass",
	Run:         runTypeToEffectSchemaClass,
}

func runTypeToEffectSchemaClass(ctx *refactor.Context) []ls.CodeAction {
	matchedNode := findInterfaceOrTypeAlias(ctx)
	if matchedNode == nil {
		return nil
	}

	// Schema.Class is not applicable when the type has index signatures
	if schemagen.HasIndexSignatures(matchedNode) {
		return nil
	}

	version := ctx.TypeParser.SupportedEffectVersion()

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Generate Schema.Class from type",
		Run: func(tracker *rewriter.Tracker) {
			gen := schemagen.New(tracker, ctx.SourceFile, version)
			newNode := gen.Process(matchedNode, true)
			if newNode != nil {
				tracker.InsertNodeBefore(ctx.SourceFile, matchedNode, newNode, true, rewriter.LeadingTriviaOptionNone)
			}
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.typeToEffectSchemaClass"
	return []ls.CodeAction{*action}
}
