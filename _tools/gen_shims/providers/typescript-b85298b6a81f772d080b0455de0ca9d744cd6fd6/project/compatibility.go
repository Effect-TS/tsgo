package project

import (
	"github.com/microsoft/typescript-go/internal/project"
	"github.com/microsoft/typescript-go/internal/tspath"
)

func NewOpenFileSnapshotRequest(fileName string, currentDirectory string, useCaseSensitiveFileNames bool) *project.APISnapshotRequest {
	file := tspath.GetNormalizedAbsolutePath(fileName, currentDirectory)
	return &project.APISnapshotRequest{OpenFiles: map[tspath.Path]string{
		tspath.ToPath(file, currentDirectory, useCaseSensitiveFileNames): file,
	}}
}

func DerefSnapshot(snapshot *project.Snapshot, _ *project.Session) {
	snapshot.Deref()
}
