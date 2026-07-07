package effecttest_test

import (
	"cmp"
	"slices"
	"testing"

	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestAutoImportEffectStyleConsistency_namespace(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["EFFECT"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
runPromiseExit/*completion*/();
// @Filename: /mainFix.ts
runPromiseExit/*fix*/();
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	completion := "completion"

	f.VerifyApplyCodeActionFromCompletion(t, &completion, &fourslash.ApplyCodeActionFromCompletionOptions{
		Name:        "runPromiseExit",
		Source:      "effect/Effect",
		Description: "Add import from \"effect/Effect\"",
		NewFileContent: new(`import * as Effect from "effect/Effect";

Effect.runPromiseExit();`),
		UserPreferences: preferences,
	})

	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as Effect from "effect/Effect";

Effect.runPromiseExit();
`}, preferences)
}

func TestAutoImportEffectStyleConsistency_barrel(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["EFFECT"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	f.GoToMarker(t, "fix")
	// Barrel imports use the top-level effect package while preserving the
	// namespace-qualified call shape from the direct module imports.
	verifyImportFixContentsUnordered(t, f, "fix", "succeed(1);\n", []string{
		"import { Channel } from \"effect\";\n\nChannel.succeed(1);\n",
		"import { Config } from \"effect\";\n\nConfig.succeed(1);\n",
		"import { Deferred } from \"effect\";\n\nDeferred.succeed(1);\n",
		"import { DurableDeferred } from \"effect\";\n\nDurableDeferred.succeed(1);\n",
		"import { Effect } from \"effect\";\n\nEffect.succeed(1);\n",
		"import { Exit } from \"effect\";\n\nExit.succeed(1);\n",
		"import { Layer } from \"effect\";\n\nLayer.succeed(1);\n",
		"import { Prompt } from \"effect\";\n\nPrompt.succeed(1);\n",
		"import { Request } from \"effect\";\n\nRequest.succeed(1);\n",
		"import { Result } from \"effect\";\n\nResult.succeed(1);\n",
		"import { SchemaGetter } from \"effect\";\n\nSchemaGetter.succeed(1);\n",
		"import { Sink } from \"effect\";\n\nSink.succeed(1);\n",
		"import { Stream } from \"effect\";\n\nStream.succeed(1);\n",
		"import { TxDeferred } from \"effect\";\n\nTxDeferred.succeed(1);\n",
	}, preferences)
}

func verifyImportFixContentsUnordered(t *testing.T, f *fourslash.FourslashTest, markerName string, originalContent string, expected []string, preferences *lsutil.UserPreferences) {
	t.Helper()
	f.GoToMarker(t, markerName)
	if preferences != nil {
		reset := f.ConfigureWithReset(t, *preferences)
		defer reset()
	}

	marker := f.MarkerByName(t, markerName)
	uri := lsconv.FileNameToDocumentURI(marker.FileName())
	client := fourslash.FourslashTest_client(f)

	diagIDValue := client.NextID()
	diagID := lsproto.NewID(lsproto.IntegerOrString{Integer: &diagIDValue})
	diagReq := lsproto.TextDocumentDiagnosticInfo.NewRequestMessage(diagID, &lsproto.DocumentDiagnosticParams{
		TextDocument: lsproto.TextDocumentIdentifier{Uri: uri},
	})
	diagResp, ok := client.SendRequestWorker(t, diagReq, diagID)
	if !ok {
		t.Fatal("diagnostic request failed")
	}
	diagResult, err := lsproto.TextDocumentDiagnosticInfo.UnmarshalResult(diagResp.Result)
	if err != nil {
		t.Fatalf("failed to unmarshal diagnostic response: %v", err)
	}

	var diagnostics []*lsproto.Diagnostic
	if diagResult.FullDocumentDiagnosticReport != nil && diagResult.FullDocumentDiagnosticReport.Items != nil {
		diagnostics = diagResult.FullDocumentDiagnosticReport.Items
	}

	actionIDValue := client.NextID()
	actionID := lsproto.NewID(lsproto.IntegerOrString{Integer: &actionIDValue})
	actionReq := lsproto.TextDocumentCodeActionInfo.NewRequestMessage(actionID, &lsproto.CodeActionParams{
		TextDocument: lsproto.TextDocumentIdentifier{Uri: uri},
		Range:        lsproto.Range{Start: marker.LSPos(), End: marker.LSPos()},
		Context:      &lsproto.CodeActionContext{Diagnostics: diagnostics},
	})
	actionResp, ok := client.SendRequestWorker(t, actionReq, actionID)
	if !ok {
		t.Fatal("code action request failed")
	}
	actionResult, err := lsproto.TextDocumentCodeActionInfo.UnmarshalResult(actionResp.Result)
	if err != nil {
		t.Fatalf("failed to unmarshal code action response: %v", err)
	}

	var actual []string
	lineMap := lsconv.ComputeLSPLineStarts(originalContent)
	if actionResult.CommandOrCodeActionArray != nil {
		for _, item := range *actionResult.CommandOrCodeActionArray {
			if item.CodeAction == nil || item.CodeAction.Kind == nil || *item.CodeAction.Kind != lsproto.CodeActionKindQuickFix {
				continue
			}
			if item.CodeAction.Edit == nil || item.CodeAction.Edit.Changes == nil {
				continue
			}
			for _, edits := range *item.CodeAction.Edit.Changes {
				actual = append(actual, applyTextEdits(originalContent, lineMap, edits))
			}
		}
	}

	slices.Sort(actual)
	expected = slices.Clone(expected)
	slices.Sort(expected)
	if !slices.Equal(actual, expected) {
		t.Fatalf("Unexpected import fix contents.\nExpected: %v\nActual: %v", expected, actual)
	}
}

func applyTextEdits(original string, lineMap *lsconv.LSPLineMap, edits []*lsproto.TextEdit) string {
	sortedEdits := slices.Clone(edits)
	slices.SortFunc(sortedEdits, func(a, b *lsproto.TextEdit) int {
		if a.Range.Start.Line != b.Range.Start.Line {
			return cmp.Compare(int(b.Range.Start.Line), int(a.Range.Start.Line))
		}
		if a.Range.Start.Character != b.Range.Start.Character {
			return cmp.Compare(int(b.Range.Start.Character), int(a.Range.Start.Character))
		}
		if a.Range.End.Line != b.Range.End.Line {
			return cmp.Compare(int(b.Range.End.Line), int(a.Range.End.Line))
		}
		if a.Range.End.Character != b.Range.End.Character {
			return cmp.Compare(int(b.Range.End.Character), int(a.Range.End.Character))
		}
		return cmp.Compare(len(a.NewText), len(b.NewText))
	})

	content := original
	for _, edit := range sortedEdits {
		start := lspPositionToOffset(content, lineMap, edit.Range.Start)
		end := lspPositionToOffset(content, lineMap, edit.Range.End)
		content = content[:start] + edit.NewText + content[end:]
		lineMap = lsconv.ComputeLSPLineStarts(content)
	}
	return content
}

func lspPositionToOffset(text string, lineMap *lsconv.LSPLineMap, pos lsproto.Position) int {
	lineStart := int(lineMap.LineStarts[pos.Line])
	lineText := text[lineStart:]
	for i, r := range lineText {
		if pos.Character == 0 {
			return lineStart + i
		}
		if r == '\n' || r == '\r' {
			return lineStart + i
		}
		pos.Character--
	}
	return len(text)
}

func TestAutoImportEffectStyleConsistency_topLevelNamedReexportsIgnore(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "topLevelNamedReexports": "ignore"
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	completion := "completion"
	_ = completion

	f.GoToMarker(t, "fix")
	// With the real effect-v4 package mounted, succeed is surfaced from multiple
	// namespace modules rather than as a direct named import from "effect".
	f.VerifyImportFixAtPosition(t, []string{
		"import * as Channel from \"effect/Channel\";\n\nChannel.succeed(1);\n",
		"import * as Config from \"effect/Config\";\n\nConfig.succeed(1);\n",
		"import * as Deferred from \"effect/Deferred\";\n\nDeferred.succeed(1);\n",
		"import * as Effect from \"effect/Effect\";\n\nEffect.succeed(1);\n",
		"import * as Exit from \"effect/Exit\";\n\nExit.succeed(1);\n",
		"import * as Layer from \"effect/Layer\";\n\nLayer.succeed(1);\n",
		"import * as Request from \"effect/Request\";\n\nRequest.succeed(1);\n",
		"import * as Result from \"effect/Result\";\n\nResult.succeed(1);\n",
		"import * as SchemaGetter from \"effect/SchemaGetter\";\n\nSchemaGetter.succeed(1);\n",
		"import * as Sink from \"effect/Sink\";\n\nSink.succeed(1);\n",
		"import * as Stream from \"effect/Stream\";\n\nStream.succeed(1);\n",
		"import * as TxDeferred from \"effect/TxDeferred\";\n\nTxDeferred.succeed(1);\n",
		"import * as Prompt from \"effect/unstable/cli/Prompt\";\n\nPrompt.succeed(1);\n",
		"import * as DurableDeferred from \"effect/unstable/workflow/DurableDeferred\";\n\nDurableDeferred.succeed(1);\n",
	}, preferences)
}

func TestAutoImportEffectStyleConsistency_topLevelNamedReexportsFollow(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "topLevelNamedReexports": "follow"
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
runPromiseExit/*completion*/();
// @Filename: /mainFix.ts
runPromiseExit/*fix*/();
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}

	// With topLevelNamedReexports="follow", the reexport from "effect" is suppressed,
	// leaving only the direct namespace import from "effect/Effect".
	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as Effect from "effect/Effect";

Effect.runPromiseExit();
`}, preferences)
}

func TestAutoImportEffectStyleConsistency_testClockWithNamespace(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
testClockWith/*completion*/(() => undefined as any);
// @Filename: /mainFix.ts
testClockWith/*fix*/(() => undefined as any);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	completion := "completion"

	f.VerifyApplyCodeActionFromCompletion(t, &completion, &fourslash.ApplyCodeActionFromCompletionOptions{
		Name:        "testClockWith",
		Source:      "effect/testing/TestClock",
		Description: "Add import from \"effect/testing/TestClock\"",
		NewFileContent: new(`import * as TestClock from "effect/testing/TestClock";

TestClock.testClockWith(() => undefined as any);`),
		UserPreferences: preferences,
	})

	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as TestClock from "effect/testing/TestClock";

TestClock.testClockWith(() => undefined as any);
`}, preferences)
}

func TestAutoImportEffectStyleConsistency_testClockWithNamespaceAlongsideNamedImport(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainFix.ts
import { adjust } from "effect/testing/TestClock"

void adjust
testClockWith/*fix*/(() => undefined as any);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}

	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as TestClock from "effect/testing/TestClock";
import { adjust } from "effect/testing/TestClock"

void adjust
TestClock.testClockWith(() => undefined as any);
`}, preferences)
}

func TestAutoImportEffectStyleConsistency_testClockWithUsesExistingNamespaceImport(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainFix.ts
import * as TestClock from "effect/testing/TestClock";

void TestClock.adjust
testClockWith/*fix*/(() => undefined as any);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}

	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as TestClock from "effect/testing/TestClock";

void TestClock.adjust
TestClock.testClockWith(() => undefined as any);
`}, preferences)
}
