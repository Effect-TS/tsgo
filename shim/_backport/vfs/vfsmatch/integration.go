package vfsmatch

import (
	"github.com/microsoft/TypeScript/tsc/providershim/tspath"
	"github.com/microsoft/TypeScript/tsc/providershim/vfs/vfsmatch"
)

func NewSpecMatcher(specs []string, basePath tspath.RootedDirectoryPath, usage vfsmatch.Usage, sensitivity tspath.CaseSensitivity) *vfsmatch.SpecMatcher {
	return vfsmatch.NewSpecMatcher(specs, basePath, usage, sensitivity)
}
