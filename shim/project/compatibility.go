package project

import (
	"github.com/microsoft/TypeScript/tsc/internal/project"
	"github.com/microsoft/TypeScript/tsc/internal/tspath"
)

func NewOpenFileSnapshotRequest(fileName string, currentDirectory string, useCaseSensitiveFileNames bool) *project.APISnapshotRequest {
	return &project.APISnapshotRequest{
		OpenFiles: map[tspath.Path]string{
			tspath.ToPath(fileName, currentDirectory, useCaseSensitiveFileNames): fileName,
		},
	}
}

func DerefSnapshot(snapshot *project.Snapshot, _ *project.Session) {
	snapshot.Deref()
}
