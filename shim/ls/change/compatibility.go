package change

import (
	"github.com/microsoft/typescript-go/internal/ls/change"
	"github.com/microsoft/typescript-go/internal/lsp/lsproto"
)

func GetChanges(tracker *change.Tracker) map[string][]*lsproto.TextEdit {
	return tracker.GetChanges()
}
