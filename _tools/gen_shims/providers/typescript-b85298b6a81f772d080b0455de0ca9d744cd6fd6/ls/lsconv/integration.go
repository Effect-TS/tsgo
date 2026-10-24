package lsconv

import (
	"github.com/microsoft/TypeScript/tsc/internal/core"
	"github.com/microsoft/TypeScript/tsc/internal/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/internal/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/internal/spanmap"
)

func FromLSPRangeToOriginal(c *lsconv.Converters, script lsconv.Script, textRange lsproto.Range) core.TextRange {
	return c.FromLSPRangeToOriginal(script, textRange)
}

func ToLSPPosition(c *lsconv.Converters, script lsconv.Script, position core.TextPos) (lsproto.Position, spanmap.Fidelity) {
	return c.ToLSPPosition(script, position)
}
