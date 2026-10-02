package project

import (
	"github.com/microsoft/typescript-go/internal/project"
	"github.com/microsoft/typescript-go/internal/tspath"
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
