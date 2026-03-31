package refactor

import (
	"context"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// Context bundles the refactor request data and provides helpers for refactor implementations.
// Unlike fixable.Context, there is no ErrorCode — refactors apply to any selection.
type Context struct {
	SourceFile *ast.SourceFile
	Span       core.TextRange
	Program    *compiler.Program
	Checker    *checker.Checker
	TypeParser typeparser.TypeParser

	Context context.Context
	ls      *ls.LanguageService
}

// NewContext creates a refactor Context from the refactor provider callback parameters.
func NewContext(ctx context.Context, sourceFile *ast.SourceFile, span core.TextRange, program *compiler.Program, langService *ls.LanguageService, checker *checker.Checker, tp typeparser.TypeParser) *Context {
	if program == nil {
		panic("refactor.NewContext: nil program")
	}
	if checker == nil {
		panic("refactor.NewContext: nil checker")
	}
	return &Context{
		SourceFile: sourceFile,
		Span:       span,
		Program:    program,
		Checker:    checker,
		TypeParser: tp,
		Context:    ctx,
		ls:         langService,
	}
}

// BytePosToLSPPosition converts a single byte offset in the context's SourceFile
// to an lsproto.Position using ECMA line/character position.
func (c *Context) BytePosToLSPPosition(pos int) lsproto.Position {
	ln, ch := scanner.GetECMALineAndUTF16CharacterOfPosition(c.SourceFile, pos)
	return lsproto.Position{Line: uint32(ln), Character: uint32(ch)}
}

// FormatOptions returns the format code settings from the language service.
func (c *Context) FormatOptions() *lsutil.FormatCodeSettings {
	return c.ls.FormatOptions()
}

// RefactorAction describes a single refactoring action to produce.
type RefactorAction struct {
	Description string
	Run         func(tracker *change.Tracker)
}

// NewRefactorAction creates a tracker, runs the action's edit closure, and returns
// a *ls.CodeAction wrapping the resulting edits for the current SourceFile.
// Returns nil if the closure produced no edits.
func (c *Context) NewRefactorAction(action RefactorAction) *ls.CodeAction {
	tracker := change.NewTracker(
		c.Context,
		c.Program.Options(),
		c.ls.FormatOptions(),
		ls.LanguageService_converters(c.ls),
	)
	action.Run(tracker)
	edits := tracker.GetChanges()[c.SourceFile.FileName()]
	if len(edits) == 0 {
		return nil
	}
	return &ls.CodeAction{
		Description: action.Description,
		Changes:     edits,
	}
}
