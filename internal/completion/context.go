package completion

import (
	"context"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// Context bundles the completion request data and provides helpers for completion implementations.
// It provides access to the source file, cursor position, existing completion items,
// and type checker access via GetTypeCheckerForFile.
type Context struct {
	SourceFile    *ast.SourceFile
	Position      int
	ExistingItems []*lsproto.CompletionItem

	ctx     context.Context
	program *compiler.Program
	ls      *ls.LanguageService
}

// NewContext creates a completion Context from the completion callback parameters.
func NewContext(ctx context.Context, sourceFile *ast.SourceFile, position int, existingItems []*lsproto.CompletionItem, program *compiler.Program, langService *ls.LanguageService) *Context {
	return &Context{
		SourceFile:    sourceFile,
		Position:      position,
		ExistingItems: existingItems,
		ctx:           ctx,
		program:       program,
		ls:            langService,
	}
}

// GetTypeCheckerForFile returns the type checker for the given source file and a cleanup function.
// Callers must defer the cleanup function.
// It calls GetDiagnostics on the returned checker to ensure checkSourceFile has run,
// so that type-checked state is populated.
func (c *Context) GetTypeCheckerForFile(sf *ast.SourceFile) (*checker.Checker, func()) {
	ch, done := c.program.GetTypeCheckerForFile(c.ctx, sf)
	if ch != nil {
		ch.GetDiagnostics(c.ctx, sf)
	}
	return ch, done
}
