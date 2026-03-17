package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/schemagen"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var StructuralTypeToSchema = refactor.Refactor{
	Name:        "structuralTypeToSchema",
	Description: "Refactor to Schema (Structural)",
	Kind:        "rewrite.effect.structuralTypeToSchema",
	Run:         runStructuralTypeToSchema,
}

func runStructuralTypeToSchema(ctx *refactor.Context) []ls.CodeAction {
	matchedNode := findInterfaceOrTypeAlias(ctx)
	if matchedNode == nil {
		return nil
	}

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	version := typeparser.SupportedEffectVersion(c)

	// Get the name node and resolve the type
	var nameNode *ast.Node
	switch matchedNode.Kind {
	case ast.KindInterfaceDeclaration:
		nameNode = matchedNode.AsInterfaceDeclaration().Name()
	case ast.KindTypeAliasDeclaration:
		nameNode = matchedNode.AsTypeAliasDeclaration().Name()
	}
	if nameNode == nil {
		return nil
	}

	t := checkerutils.GetTypeAtLocation(c, nameNode)
	if t == nil {
		return nil
	}

	typeName := nameNode.AsIdentifier().Text
	isExported := ast.GetCombinedModifierFlags(matchedNode)&ast.ModifierFlagsExport != 0

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Refactor to Schema (Recursive Structural)",
		Run: func(tracker *change.Tracker) {
			gen := schemagen.NewStructuralSchemaGen(tracker, ctx.SourceFile, c, version)
			typeMap := map[string]*checker.Type{typeName: t}
			stmts := gen.Process(typeMap, matchedNode, isExported)
			for i := len(stmts) - 1; i >= 0; i-- {
				tracker.InsertNodeBefore(ctx.SourceFile, matchedNode, stmts[i], true, change.LeadingTriviaOptionNone)
			}
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.structuralTypeToSchema"
	return []ls.CodeAction{*action}
}
