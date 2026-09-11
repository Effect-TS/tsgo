package project

import "github.com/microsoft/typescript-go/internal/project"

func DerefSnapshot(snapshot *project.Snapshot, _ *project.Session) {
	snapshot.Deref()
}
