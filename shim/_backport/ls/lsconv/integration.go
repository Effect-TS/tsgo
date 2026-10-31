package lsconv

import (
	"github.com/microsoft/TypeScript/tsc/providershim/core"
	"github.com/microsoft/TypeScript/tsc/providershim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/providershim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/providershim/spanmap"
)

func FromLSPRangeToOriginal(c *lsconv.Converters, script lsconv.Script, textRange lsproto.Range) core.TextRange {
	return c.FromLSPRangeToOriginal(script, textRange)
}

func ToLSPPosition(c *lsconv.Converters, script lsconv.Script, position core.TextPos) (lsproto.Position, spanmap.Fidelity) {
	return c.ToLSPPosition(script, position)
}
