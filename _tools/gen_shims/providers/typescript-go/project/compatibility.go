package project

import (
	"github.com/microsoft/typescript-go/internal/collections"
	"github.com/microsoft/typescript-go/internal/ls/lsconv"
	"github.com/microsoft/typescript-go/internal/lsp/lsproto"
	"github.com/microsoft/typescript-go/internal/project"
)

func NewOpenFileSnapshotRequest(fileName string, _ string, _ bool) *project.APISnapshotRequest {
	openFiles := &collections.Set[lsproto.DocumentUri]{}
	openFiles.Add(lsconv.FileNameToDocumentURI(fileName))
	return &project.APISnapshotRequest{OpenFiles: openFiles}
}

func DerefSnapshot(snapshot *project.Snapshot, session *project.Session) {
	snapshot.Deref(session)
}
