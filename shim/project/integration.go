package project

import (
	"github.com/microsoft/TypeScript/tsc/internal/project"
	"github.com/microsoft/TypeScript/tsc/internal/tspath"
)

func NewOpenFileSnapshotRequest(fileName string, currentDirectory string, useCaseSensitiveFileNames bool) *project.APISnapshotRequest {
	sensitivity := tspath.CaseInsensitive
	if useCaseSensitiveFileNames {
		sensitivity = tspath.CaseSensitive
	}
	file := tspath.ToRootedFilePath(fileName, tspath.RootedDirectoryPathFromAbsolute(currentDirectory))
	return &project.APISnapshotRequest{OpenFiles: map[tspath.PathKey]tspath.RootedFilePath{
		sensitivity.PathKey(file.AsPath()): file,
	}}
}

func DerefSnapshot(snapshot *project.Snapshot, _ *project.Session) {
	snapshot.Deref()
}
