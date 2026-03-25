package effecttest_test

import (
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"unicode"
	"unicode/utf8"

	effecttest "github.com/effect-ts/effect-typescript-go/internal/effecttest"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/testutil/baseline"
)

func verifyLocalBaselineInlayHints(t *testing.T, f *fourslash.FourslashTest, testContent string, fileName string, preferences *lsutil.UserPreferences) {
	t.Helper()

	if preferences == nil {
		preferences = lsutil.NewDefaultUserPreferences()
	}
	reset := f.ConfigureWithReset(t, preferences)
	defer reset()

	fileContent := mustFindFourslashFileContent(t, testContent, fileName)
	lineMap := lsconv.ComputeLSPLineStarts(fileContent)
	end := lsproto.Position{Line: uint32(len(lineMap.LineStarts) - 1), Character: 0}
	lastLineStart := lineMap.LineStarts[len(lineMap.LineStarts)-1]
	for _, r := range fileContent[lastLineStart:] {
		end.Character += uint32(utf8.RuneLen(r))
	}

	params := &lsproto.InlayHintParams{
		TextDocument: lsproto.TextDocumentIdentifier{Uri: lsconv.FileNameToDocumentURI(fileName)},
		Range: lsproto.Range{
			Start: lsproto.Position{Line: 0, Character: 0},
			End:   end,
		},
	}

	client := fourslash.FourslashTest_client(f)
	id := client.NextID()
	reqID := lsproto.NewID(lsproto.IntegerOrString{Integer: &id})
	req := lsproto.TextDocumentInlayHintInfo.NewRequestMessage(reqID, params)
	resp, ok := client.SendRequestWorker(t, req, reqID)
	if !ok || resp == nil {
		t.Fatalf("Nil response received for %s request", lsproto.MethodTextDocumentInlayHint)
	}
	if resp.Error != nil {
		t.Fatalf("%s request returned error: %s", lsproto.MethodTextDocumentInlayHint, resp.Error.String())
	}

	result, ok := resp.Result.(lsproto.InlayHintResponse)
	if !ok {
		t.Fatalf("Unexpected %s response type: %T", lsproto.MethodTextDocumentInlayHint, resp.Result)
	}

	actual := formatInlayHintBaseline(t, fileContent, result)
	runLocalInlayBaseline(t, localInlayBaselineFileName(t), actual)
}

func mustFindFourslashFileContent(t *testing.T, testContent string, fileName string) string {
	t.Helper()

	parsed := fourslash.ParseTestData(t, testContent, localInlayBaselineFileName(t)+".ts")
	for _, file := range parsed.Files {
		if file.FileName() == fileName {
			return file.Content
		}
	}
	t.Fatalf("fourslash file %s not found", fileName)
	return ""
}

func formatInlayHintBaseline(t *testing.T, fileContent string, result lsproto.InlayHintResponse) string {
	t.Helper()

	fileLines := strings.Split(fileContent, "\n")
	annotations := []string{}
	if result.InlayHints != nil {
		slices.SortFunc(*result.InlayHints, func(a, b *lsproto.InlayHint) int {
			return lsproto.ComparePositions(a.Position, b.Position)
		})
		for _, hint := range *result.InlayHints {
			if hint.Label.InlayHintLabelParts != nil {
				for _, part := range *hint.Label.InlayHintLabelParts {
					if part.Location != nil && isLibFile(part.Location.Uri.FileName()) {
						part.Location.Range.Start = lsproto.Position{Line: 0, Character: 0}
						part.Location.Range.End = lsproto.Position{Line: 0, Character: 0}
					}
				}
			}
			underline := strings.Repeat(" ", int(hint.Position.Character)) + "^"
			hintJSON, err := core.StringifyJson(hint, "", "  ")
			if err != nil {
				t.Fatalf("Failed to stringify inlay hint for baseline: %v", err)
			}
			annotation := fileLines[hint.Position.Line]
			annotation += "\n" + underline + "\n" + hintJSON
			annotations = append(annotations, annotation)
		}
	}

	if len(annotations) == 0 {
		annotations = append(annotations, "=== No inlay hints ===")
	}

	return strings.Join(annotations, "\n\n")
}

func runLocalInlayBaseline(t *testing.T, fileName string, actual string) {
	t.Helper()

	localPath := filepath.Join(effecttest.BaselineLocalPath("inlayHints"), fileName)
	referencePath := filepath.Join(effecttest.BaselineReferencePath("inlayHints"), fileName)
	referenceContent, err := os.ReadFile(referencePath)
	if err != nil {
		if os.IsNotExist(err) {
			if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
				t.Fatalf("Failed to create local baseline directory: %v", err)
			}
			if err := os.WriteFile(localPath, []byte(actual), 0o644); err != nil {
				t.Fatalf("Failed to write local baseline: %v", err)
			}
			if err := os.MkdirAll(filepath.Dir(referencePath), 0o755); err != nil {
				t.Fatalf("Failed to create reference baseline directory: %v", err)
			}
			if err := os.WriteFile(referencePath, []byte(actual), 0o644); err != nil {
				t.Fatalf("Failed to write reference baseline: %v", err)
			}
			return
		}
		t.Fatalf("Failed to read reference baseline: %v", err)
	}

	expected := string(referenceContent)
	if actual == expected {
		return
	}
	if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
		t.Fatalf("Failed to create local baseline directory: %v", err)
	}
	if err := os.WriteFile(localPath, []byte(actual), 0o644); err != nil {
		t.Fatalf("Failed to write local baseline: %v", err)
	}
	diff := baseline.DiffText(referencePath, localPath, expected, actual)
	diffLines := strings.Split(diff, "\n")
	for i := range diffLines {
		diffLines[i] = "  " + diffLines[i]
	}
	t.Errorf("Baseline mismatch:\n%s", strings.Join(diffLines, "\n"))
}

func localInlayBaselineFileName(t *testing.T) string {
	t.Helper()
	name := t.Name()
	if i := strings.LastIndex(name, "/"); i >= 0 {
		name = name[i+1:]
	}
	name = strings.TrimPrefix(name, "Test")
	if name == "" {
		return "inlayHints.baseline"
	}
	r, size := utf8.DecodeRuneInString(name)
	return string(unicode.ToLower(r)) + name[size:] + ".baseline"
}

func isLibFile(fileName string) bool {
	baseName := filepath.Base(fileName)
	return strings.HasPrefix(baseName, "lib.") && strings.HasSuffix(baseName, ".d.ts")
}
