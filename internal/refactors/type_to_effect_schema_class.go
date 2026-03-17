package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/schemagen"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
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

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	version := typeparser.SupportedEffectVersion(c)

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Generate Schema.Class from type",
		Run: func(tracker *change.Tracker) {
			gen := schemagen.New(tracker, ctx.SourceFile, version)
			newNode := gen.Process(matchedNode, true)
			if newNode != nil {
				tracker.InsertNodeBefore(ctx.SourceFile, matchedNode, newNode, true, change.LeadingTriviaOptionNone)
			}
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.typeToEffectSchemaClass"
	return []ls.CodeAction{*action}
}
