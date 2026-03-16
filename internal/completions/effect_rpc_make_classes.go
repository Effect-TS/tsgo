package completions

import (
	"fmt"

	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// rpcMakeClasses provides completion items for @effect/rpc Rpc.make
// when the cursor is in the extends clause of a class declaration.
// This is a V3-only completion.
var rpcMakeClasses = completion.Completion{
	Name:        "rpcMakeClasses",
	Description: "Provides @effect/rpc Rpc.make completions in extends clauses",
	Run:         runRpcMakeClasses,
}

func runRpcMakeClasses(ctx *completion.Context) []*lsproto.CompletionItem {
	data := completion.ParseDataForExtendsClassCompletion(ctx.SourceFile, ctx.Position)
	if data == nil {
		return nil
	}

	// Get checker for version detection
	ch, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	defer done()

	// V3 only
	version := typeparser.SupportedEffectVersion(ch)
	if version != typeparser.EffectMajorV3 {
		return nil
	}

	rpcIdentifier := effectutil.FindModuleIdentifierForPackage(ctx.SourceFile, "@effect/rpc", "Rpc")
	accessedText := data.AccessedObjectText()

	// Only fully-qualified case (e.g., Rpc.make)
	if rpcIdentifier != accessedText {
		return nil
	}

	className := data.ClassNameText()

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	insertText := fmt.Sprintf(`%s.make("%s", {${0}}) {}`, rpcIdentifier, className)

	return []*lsproto.CompletionItem{
		makeExtendsCompletionItem(accessedText,
			fmt.Sprintf(`make("%s")`, className),
			insertText, sortText, replacementRange,
		),
	}
}
