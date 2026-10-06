package vfsmatch

import (
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs/vfsmatch"
)

func NewSpecMatcher(specs []string, basePath tspath.RootedDirectoryPath, usage vfsmatch.Usage, sensitivity tspath.CaseSensitivity) *vfsmatch.SpecMatcher {
	return vfsmatch.NewSpecMatcher(specs, basePath, usage, sensitivity == tspath.CaseSensitive)
}
