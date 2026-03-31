package completion

import (
	"context"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// Context bundles the completion request data and provides helpers for completion implementations.
// It provides access to the source file, cursor position, existing completion items,
type Context struct {
	SourceFile    *ast.SourceFile
	Position      int
	ExistingItems []*lsproto.CompletionItem
	Program       *compiler.Program
	Checker       *checker.Checker
	TypeParser    typeparser.TypeParser

	Context context.Context
	ls      *ls.LanguageService
}

// NewContext creates a completion Context from the completion callback parameters.
func NewContext(ctx context.Context, sourceFile *ast.SourceFile, position int, existingItems []*lsproto.CompletionItem, program *compiler.Program, langService *ls.LanguageService, checker *checker.Checker, tp typeparser.TypeParser) *Context {
	if program == nil {
		panic("completion.NewContext: nil program")
	}
	if checker == nil {
		panic("completion.NewContext: nil checker")
	}
	return &Context{
		SourceFile:    sourceFile,
		Position:      position,
		ExistingItems: existingItems,
		Program:       program,
		Checker:       checker,
		TypeParser:    tp,
		Context:       ctx,
		ls:            langService,
	}
}
