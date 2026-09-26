package etsoxlintrunner_test

import (
	"slices"
	"testing"

	_ "github.com/effect-ts/tsgo/etsoxlintrunner"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/vfs"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

type configHost struct{ fs vfs.FS }

func (h *configHost) FS() vfs.FS                  { return h.fs }
func (h *configHost) GetCurrentDirectory() string { return "/" }

func TestEffectFnOptionsWithExtends(t *testing.T) {
	t.Parallel()
	const options = `{"plugins":[{"name":"@effect/language-service","effectFn":["span","inferred-span","suggested-span"]}]}`
	for _, tt := range []struct {
		name, config, base string
	}{
		{"inline", `{"compilerOptions":` + options + `,"files":["main.ts"]}`, `{}`},
		{"inherited", `{"extends":"./base.json","files":["main.ts"]}`, `{"compilerOptions":` + options + `}`},
		{"local", `{"extends":"./base.json","compilerOptions":` + options + `,"files":["main.ts"]}`, `{}`},
	} {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			host := &configHost{fs: vfstest.FromMap(map[string]any{
				"/main.ts":       "export {}",
				"/base.json":     tt.base,
				"/tsconfig.json": tt.config,
			}, true)}
			source := tsoptions.NewTsconfigSourceFileFromFilePath("/tsconfig.json", "/tsconfig.json", tt.config)
			parsed := tsoptions.ParseJsonSourceFileConfigFileContent(source, host, "/", nil, nil, "/tsconfig.json", nil, nil, nil)
			if len(parsed.Errors) != 0 {
				t.Fatalf("unexpected config errors: %v", parsed.Errors)
			}
			if parsed.CompilerOptions().Effect == nil {
				t.Fatal("Effect plugin options were lost")
			}
			if got := parsed.CompilerOptions().Effect.EffectFn; !slices.Equal(got, []string{"span", "inferred-span", "suggested-span"}) {
				t.Fatalf("unexpected effectFn options: %v", got)
			}
		})
	}
}
