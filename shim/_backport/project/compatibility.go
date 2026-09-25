package project

import (
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
)

func NewOpenFileSnapshotRequest(fileName string, _ string, _ bool) *project.APISnapshotRequest {
	openFiles := &collections.Set[lsproto.DocumentUri]{}
	openFiles.Add(lsconv.FileNameToDocumentURI(fileName))
	return &project.APISnapshotRequest{OpenFiles: openFiles}
}

func DerefSnapshot(snapshot *project.Snapshot, session *project.Session) {
	snapshot.Deref(session)
}
