package etsapi

import (
	"context"

	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/schemagen"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// RenderStructuralSchemaStatements converts a resolved TypeScript type into
// rendered Effect Schema statements using the same structural generator as the
// structuralTypeToSchema refactor.
func (tp *TypeParser) RenderStructuralSchemaStatements(sourceFile *ast.SourceFile, name string, t *checker.Type, isExported bool) []string {
	if tp == nil || tp.inner == nil || tp.program == nil || tp.checker == nil || sourceFile == nil || name == "" || t == nil {
		return nil
	}

	converters := lsconv.NewConverters(lsproto.PositionEncodingKindUTF8, func(fileName string) *lsconv.LSPLineMap {
		if fileName == sourceFile.FileName() {
			return lsconv.ComputeLSPLineStarts(sourceFile.Text())
		}
		return nil
	})
	rawTracker := change.NewTracker(context.Background(), tp.program.Options(), lsutil.GetDefaultFormatCodeSettings(), converters)
	tracker := rewriter.NewTracker(rawTracker)

	gen := schemagen.NewStructuralSchemaGen(tracker, tp.inner, sourceFile, tp.checker, tp.inner.SupportedEffectVersion())
	statements := gen.Process(map[string]*checker.Type{name: t}, sourceFile.AsNode(), isExported)
	if len(statements) == 0 {
		return nil
	}

	rendered := make([]string, 0, len(statements))
	for _, statement := range statements {
		if statement == nil {
			continue
		}
		text, _ := change.Tracker_getNonformattedText(rawTracker, statement, sourceFile)
		if text != "" {
			rendered = append(rendered, text)
		}
	}
	return rendered
}
