package etstesthooks

import (
	"strings"
	"testing"
	"testing/fstest"
)

func TestPrepareTestFSMountsEffectForV4MarkerWithoutImport(t *testing.T) {
	t.Parallel()

	testfs := map[string]any{
		"/main.ts": "// @effect-v4\nconst value = 1\n",
	}

	prepareTestFS(testfs)

	assertMountedEffect(t, testfs)
	assertSourcePackageName(t, testfs, "effect-v4-tests")
}

func TestPrepareTestFSMountsEffectForV3MarkerWithoutImport(t *testing.T) {
	t.Parallel()

	testfs := map[string]any{
		"/main.ts": "// @effect-v3\nconst value = 1\n",
	}

	prepareTestFS(testfs)

	assertMountedEffect(t, testfs)
	assertSourcePackageName(t, testfs, "effect-v3-tests")
}

func TestPrepareTestFSDoesNotMountWithoutImportOrMarker(t *testing.T) {
	t.Parallel()

	testfs := map[string]any{
		"/main.ts": "const value = 1\n",
	}

	prepareTestFS(testfs)

	if _, ok := testfs["/.src/package.json"]; ok {
		t.Fatal("expected no mounted Effect package without import or marker")
	}
	if _, ok := testfs["/node_modules/effect/package.json"]; ok {
		t.Fatal("expected no mounted effect package without import or marker")
	}
}

func assertMountedEffect(t *testing.T, testfs map[string]any) {
	t.Helper()
	if _, ok := testfs["/.src/package.json"]; !ok {
		t.Fatal("expected mounted /.src/package.json")
	}
	if _, ok := testfs["/node_modules/effect/package.json"]; !ok {
		t.Fatal("expected mounted /node_modules/effect/package.json")
	}
}

func assertSourcePackageName(t *testing.T, testfs map[string]any, want string) {
	t.Helper()
	file, ok := testfs["/.src/package.json"].(*fstest.MapFile)
	if !ok {
		t.Fatal("expected /.src/package.json to be a fstest.MapFile")
	}
	if !strings.Contains(string(file.Data), want) {
		t.Fatalf("expected /.src/package.json to contain %q, got %q", want, string(file.Data))
	}
}
