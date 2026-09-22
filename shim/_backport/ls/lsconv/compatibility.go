package lsconv

import (
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

func FromLSPRangeToOriginal(c *lsconv.Converters, script lsconv.Script, textRange lsproto.Range) core.TextRange {
	return c.FromLSPRange(script, textRange)
}
