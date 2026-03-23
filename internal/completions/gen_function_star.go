package completions

import (
	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// genFunctionStar provides a completion for `gen(function*(){})` when dot-accessing
// an object that has a callable `gen` property (e.g., `Effect.gen`, `Micro.gen`).
var genFunctionStar = completion.Completion{
	Name:        "genFunctionStar",
	Description: "Provides gen(function*(){}) completion when accessing an object with a callable gen property",
	Run:         runGenFunctionStar,
}

func runGenFunctionStar(ctx *completion.Context) []*lsproto.CompletionItem {
	result := completion.ParseAccessedExpressionForCompletion(ctx.SourceFile, ctx.Position)
	if result == nil {
		return nil
	}

	ch, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	defer done()

	t := typeparser.GetTypeAtLocation(ch, result.AccessedObject)
	if t == nil {
		return nil
	}

	genSymbol := ch.GetPropertyOfType(t, "gen")
	if genSymbol == nil {
		return nil
	}

	genType := ch.GetTypeOfSymbolAtLocation(genSymbol, result.AccessedObject)
	callSigs := ch.GetSignaturesOfType(genType, checker.SignatureKindCall)
	if len(callSigs) == 0 {
		return nil
	}

	// Replacement span: from after the dot to the cursor position
	// This matches the reference: span starts at accessedObject.end + 1 (after the dot)
	accessedEnd := result.AccessedObject.End()
	spanStart := accessedEnd + 1
	spanLength := max(ctx.Position-spanStart, 0)

	replacementRange := byteSpanToRange(ctx, spanStart, spanLength)
	sortText := "11"

	return []*lsproto.CompletionItem{
		makeCompletionItem(
			"gen(function*(){})",
			"gen(function*(){${0}})",
			sortText,
			replacementRange,
		),
	}
}
