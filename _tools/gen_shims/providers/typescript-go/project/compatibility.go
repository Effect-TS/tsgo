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

// ContentMapperSelector mirrors the TypeScript next Client.RegisterContentMapperSelectors argument. typescript-go's
// Client has no content mapper registration, so the type is only there to let one Client implementation compile
// against every supported compiler.
type ContentMapperSelector struct {
	Pattern   string
	Extension string
}
