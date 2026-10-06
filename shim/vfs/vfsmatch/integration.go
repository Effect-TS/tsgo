package vfsmatch

import (
	"github.com/microsoft/TypeScript/tsc/internal/tspath"
	"github.com/microsoft/TypeScript/tsc/internal/vfs/vfsmatch"
)

func NewSpecMatcher(specs []string, basePath tspath.RootedDirectoryPath, usage vfsmatch.Usage, sensitivity tspath.CaseSensitivity) *vfsmatch.SpecMatcher {
	return vfsmatch.NewSpecMatcher(specs, basePath, usage, sensitivity)
}
