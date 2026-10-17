package vfstest

import (
	"github.com/microsoft/typescript-go/internal/vfs"
	"github.com/microsoft/typescript-go/internal/vfs/vfstest"
)

// FromMap exposes the generic test helper with the provider's original casing API.
func FromMap(m map[string]any, caseSensitive bool) vfs.FS {
	return vfstest.FromMap(m, caseSensitive)
}

var Symlink = vfstest.Symlink
