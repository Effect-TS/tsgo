package vfstest

import (
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"
)

// FromMap creates a new vfs.FS from a map of paths to file contents.
// This is a non-generic wrapper around vfstest.FromMap for use from our shim system.
// The map values can be strings, byte slices, or *fstest.MapFile.
func FromMap(m map[string]any, sensitivity tspath.CaseSensitivity) vfs.FS {
	return vfstest.FromMap(m, sensitivity == tspath.CaseSensitive)
}

// Symlink is re-exported from the vfstest package.
var Symlink = vfstest.Symlink
