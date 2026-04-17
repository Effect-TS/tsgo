package effecttest

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
	"github.com/microsoft/typescript-go/shim/project/logging"
	"github.com/microsoft/typescript-go/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"
)

// DocumentSymbolTestCasesDir returns the path to the Effect document-symbol test cases directory.
func DocumentSymbolTestCasesDir(version bundledeffect.EffectVersion) string {
	return filepath.Join(bundledeffect.EffectTsGoRootPath(), "testdata", "tests", string(version)+"-document-symbols")
}

// DiscoverDocumentSymbolTestCases finds all .ts test files in the document-symbol test cases directory.
func DiscoverDocumentSymbolTestCases(version bundledeffect.EffectVersion) ([]string, error) {
	dir := DocumentSymbolTestCasesDir(version)
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var cases []string
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".ts" {
			cases = append(cases, filepath.Join(dir, entry.Name()))
		}
	}
	return cases, nil
}

// RunEffectDocumentSymbolsTest executes one document-symbol baseline test case.
func RunEffectDocumentSymbolsTest(t *testing.T, version bundledeffect.EffectVersion, testFile string) {
	AcquireProgram()
	defer ReleaseProgram()

	content, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatal("Failed to read test file:", err)
	}

	defaultFileName := tspath.GetBaseFileName(testFile)
	units := parseTestUnits(string(content), defaultFileName)

	testfs := make(map[string]any)
	if err := bundledeffect.MountEffect(version, testfs); err != nil {
		t.Fatal("Failed to mount Effect:", err)
	}

	currentDirectory := "/.src"
	sourceFileNames := make([]string, 0, len(units))
	hasTsConfig := false
	configDirectory := currentDirectory

	for _, unit := range units {
		unitName := tspath.GetNormalizedAbsolutePath(unit.name, currentDirectory)
		testfs[unitName] = &fstest.MapFile{Data: []byte(unit.content)}

		lowerName := strings.ToLower(unitName)
		if strings.HasSuffix(lowerName, "tsconfig.json") || strings.HasSuffix(lowerName, "jsconfig.json") {
			hasTsConfig = true
			continue
		}
		if strings.HasPrefix(unitName, "/node_modules/") {
			continue
		}
		if len(sourceFileNames) == 0 {
			configDirectory = tspath.GetDirectoryPath(unitName)
		}
		sourceFileNames = append(sourceFileNames, unitName)
	}

	if !hasTsConfig {
		unitName := tspath.CombinePaths(configDirectory, "tsconfig.json")
		testfs[unitName] = &fstest.MapFile{Data: []byte(DefaultTsConfig)}
	}

	fs := vfstest.FromMap(testfs, true /*useCaseSensitiveFileNames*/)
	fs = bundled.WrapFS(fs)

	session := project.NewSession(&project.SessionInit{
		BackgroundCtx: context.Background(),
		Options: &project.SessionOptions{
			CurrentDirectory:   currentDirectory,
			DefaultLibraryPath: bundled.LibPath(),
			TypingsLocation:    "/home/src/Library/Caches/typescript",
			PositionEncoding:   lsproto.PositionEncodingKindUTF8,
			WatchEnabled:       false,
			LoggingEnabled:     false,
		},
		FS:          fs,
		Client:      noopProjectClient{},
		Logger:      logging.NewLogger(io.Discard),
		NpmExecutor: nil,
	})

	for _, fileName := range sourceFileNames {
		content, ok := readMapFileContent(testfs[fileName])
		if !ok {
			t.Fatalf("missing file content for %s", fileName)
		}
		session.DidOpenFile(context.Background(), lsconv.FileNameToDocumentURI(fileName), 1, content, lsproto.LanguageKindTypeScript)
	}

	fileResults := make([]DocumentSymbolsFileResult, 0, len(sourceFileNames))
	for _, fileName := range sourceFileNames {
		fileResults = append(fileResults, collectDocumentSymbolsForFile(t, session, fileName))
	}

	baselineName := strings.TrimSuffix(tspath.GetBaseFileName(testFile), ".ts")
	DoDocumentSymbolsBaseline(t, baselineName, string(version), fileResults)
}

func collectDocumentSymbolsForFile(t *testing.T, session *project.Session, fileName string) DocumentSymbolsFileResult {
	t.Helper()

	uri := lsconv.FileNameToDocumentURI(fileName)
	langService, err := session.GetLanguageService(context.Background(), uri)
	if err != nil {
		t.Fatalf("failed to get language service for %s: %v", fileName, err)
	}

	hierarchical := collectHierarchicalDocumentSymbols(t, langService, uri)
	flat := collectFlatDocumentSymbols(t, langService, uri)

	return DocumentSymbolsFileResult{
		FileName:      fileName,
		DocumentTrees: [][]*lsproto.DocumentSymbol{hierarchical},
		FlatLists:     [][]*lsproto.SymbolInformation{flat},
	}
}

func collectHierarchicalDocumentSymbols(t *testing.T, langService interface {
	ProvideDocumentSymbols(ctx context.Context, documentURI lsproto.DocumentUri) (lsproto.DocumentSymbolResponse, error)
}, uri lsproto.DocumentUri,
) []*lsproto.DocumentSymbol {
	t.Helper()

	caps := &lsproto.ClientCapabilities{
		TextDocument: &lsproto.TextDocumentClientCapabilities{
			DocumentSymbol: &lsproto.DocumentSymbolClientCapabilities{
				HierarchicalDocumentSymbolSupport: &[]bool{true}[0],
			},
		},
	}
	resolvedCaps := caps.Resolve()
	ctx := lsproto.WithClientCapabilities(context.Background(), &resolvedCaps)
	result, err := langService.ProvideDocumentSymbols(ctx, uri)
	if err != nil {
		t.Fatalf("ProvideDocumentSymbols hierarchical failed for %s: %v", uri, err)
	}
	if result.DocumentSymbols == nil {
		return nil
	}
	return *result.DocumentSymbols
}

func collectFlatDocumentSymbols(t *testing.T, langService interface {
	ProvideDocumentSymbols(ctx context.Context, documentURI lsproto.DocumentUri) (lsproto.DocumentSymbolResponse, error)
}, uri lsproto.DocumentUri,
) []*lsproto.SymbolInformation {
	t.Helper()

	caps := &lsproto.ClientCapabilities{
		TextDocument: &lsproto.TextDocumentClientCapabilities{
			DocumentSymbol: &lsproto.DocumentSymbolClientCapabilities{
				HierarchicalDocumentSymbolSupport: &[]bool{false}[0],
			},
		},
	}
	resolvedCaps := caps.Resolve()
	ctx := lsproto.WithClientCapabilities(context.Background(), &resolvedCaps)
	result, err := langService.ProvideDocumentSymbols(ctx, uri)
	if err != nil {
		t.Fatalf("ProvideDocumentSymbols flat failed for %s: %v", uri, err)
	}
	if result.SymbolInformations == nil {
		return nil
	}
	return *result.SymbolInformations
}

func readMapFileContent(entry any) (string, bool) {
	switch v := entry.(type) {
	case *fstest.MapFile:
		return string(v.Data), true
	case string:
		return v, true
	default:
		return "", false
	}
}

type noopProjectClient struct{}

func (noopProjectClient) WatchFiles(_ context.Context, _ project.WatcherID, _ []*lsproto.FileSystemWatcher) error {
	return nil
}

func (noopProjectClient) UnwatchFiles(_ context.Context, _ project.WatcherID) error { return nil }

func (noopProjectClient) RefreshDiagnostics(_ context.Context) error { return nil }

func (noopProjectClient) PublishDiagnostics(_ context.Context, _ *lsproto.PublishDiagnosticsParams) error {
	return nil
}

func (noopProjectClient) RefreshInlayHints(_ context.Context) error { return nil }

func (noopProjectClient) RefreshCodeLens(_ context.Context) error { return nil }

func (noopProjectClient) ProgressStart(_ *diagnostics.Message, _ ...any) {}

func (noopProjectClient) ProgressFinish(_ *diagnostics.Message, _ ...any) {}

func (noopProjectClient) SendTelemetry(_ context.Context, _ lsproto.TelemetryEvent) error { return nil }

func (noopProjectClient) IsActive() bool { return true }
