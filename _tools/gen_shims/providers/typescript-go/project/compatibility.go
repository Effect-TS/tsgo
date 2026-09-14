package project

import "github.com/microsoft/typescript-go/internal/project"

func DerefSnapshot(snapshot *project.Snapshot, session *project.Session) {
	snapshot.Deref(session)
}
