package project

import "github.com/microsoft/TypeScript/tsc/internal/project"

func DerefSnapshot(snapshot *project.Snapshot, _ *project.Session) {
	snapshot.Deref()
}
