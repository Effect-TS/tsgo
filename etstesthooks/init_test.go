package etstesthooks

import (
	"strings"
	"testing"
	"testing/fstest"
)

func TestPrepareTestFSMountsEffect(t *testing.T) {
	t.Parallel()

	for _, tt := range []struct {
		name        string
		source      string
		mounted     bool
		packageName string
	}{
		{"v4 marker", "// @effect-v4\nconst value = 1\n", true, "effect-v4-tests"},
		{"v3 marker", "// @effect-v3\nconst value = 1\n", true, "effect-v3-tests"},
		{"no import or marker", "const value = 1\n", false, ""},
	} {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			testfs := map[string]any{"/main.ts": tt.source}

			prepareTestFS(testfs)

			sourcePackage, hasSourcePackage := testfs["/.src/package.json"]
			_, hasEffectPackage := testfs["/node_modules/effect/package.json"]
			if hasSourcePackage != tt.mounted || hasEffectPackage != tt.mounted {
				t.Fatalf("mounted /.src/package.json=%t /node_modules/effect/package.json=%t, want %t", hasSourcePackage, hasEffectPackage, tt.mounted)
			}
			if !tt.mounted {
				return
			}
			file, ok := sourcePackage.(*fstest.MapFile)
			if !ok {
				t.Fatal("expected /.src/package.json to be a fstest.MapFile")
			}
			if !strings.Contains(string(file.Data), tt.packageName) {
				t.Fatalf("expected /.src/package.json to contain %q, got %q", tt.packageName, string(file.Data))
			}
		})
	}
}
