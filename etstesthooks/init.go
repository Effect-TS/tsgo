package etstesthooks

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/fourslash"
)

func init() {
	fourslash.RegisterPrepareTestFSCallback(prepareTestFS)
}

// prepareTestFS detects Effect imports or explicit version markers in test files
// and mounts real Effect packages into the fourslash VFS.
func prepareTestFS(testfs map[string]any) {
	hasEffectImport := false
	hasV3Marker := false
	hasV4Marker := false
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
		if strings.HasPrefix(content, "// @effect-v4") || strings.Contains(content, "\n// @effect-v4") {
			hasV4Marker = true
		}
	}
	if !hasEffectImport && !hasV3Marker && !hasV4Marker {
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
