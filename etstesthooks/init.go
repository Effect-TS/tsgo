package etstesthooks

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/fourslash"
)

func init() {
	fourslash.RegisterPrepareTestFSCallback(prepareTestFS)
}

// prepareTestFS detects Effect imports in test files and mounts real Effect packages.
// It checks for a // @effect-v3 marker at the start of any file to choose the library version.
func prepareTestFS(testfs map[string]any) {
	hasEffectImport := false
	hasV3Marker := false
	for _, v := range testfs {
		content, ok := v.(string)
		if !ok {
			continue
		}
		if strings.Contains(content, `from "effect`) {
			hasEffectImport = true
		}
		if strings.HasPrefix(content, "// @effect-v3") || strings.Contains(content, "\n// @effect-v3") {
			hasV3Marker = true
		}
	}
	if !hasEffectImport {
		return
	}
	version := bundledeffect.EffectV4
	if hasV3Marker {
		version = bundledeffect.EffectV3
	}
	if err := bundledeffect.MountEffect(version, testfs); err != nil {
		panic(err)
	}
}
