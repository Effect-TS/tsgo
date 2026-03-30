package fixable

import (
	"context"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// Context bundles the code-fix request data and provides helpers for fixable implementations.
// It replaces the (context.Context, *change.Tracker, *ls.CodeFixContext) parameter triple,
// giving each fixable self-contained access to the checker, tracker lifecycle, and edit finalization.
type Context struct {
	SourceFile *ast.SourceFile
	Span       core.TextRange
	ErrorCode  int32
	Options    *etscore.ResolvedEffectPluginOptions

	ctx    context.Context
	fixCtx *ls.CodeFixContext
}

// NewContext creates a fixable Context from the standard code-fix request parameters.
func NewContext(ctx context.Context, fixCtx *ls.CodeFixContext, options *etscore.ResolvedEffectPluginOptions) *Context {
	return &Context{
		SourceFile: fixCtx.SourceFile,
		Span:       fixCtx.Span,
		ErrorCode:  fixCtx.ErrorCode,
		Options:    options,
		ctx:        ctx,
		fixCtx:     fixCtx,
	}
}

// GetTypeCheckerForFile returns the type checker for the given source file and a cleanup function.
// Callers must defer the cleanup function.
// It calls GetDiagnostics on the returned checker to ensure checkSourceFile has run,
// so that GetRelationErrors and other type-checked state is populated.
func (c *Context) GetTypeCheckerForFile(sf *ast.SourceFile) (*checker.Checker, func()) {
	ch, done := c.fixCtx.Program.GetTypeCheckerForFile(c.ctx, sf)
	if ch != nil {
		ch.GetDiagnostics(c.ctx, sf)
	}
	return ch, done
}

// BytePosToLSPPosition converts a single byte offset in the context's SourceFile
// to an lsproto.Position using ECMA line/character position.
func (c *Context) BytePosToLSPPosition(pos int) lsproto.Position {
	ln, ch := scanner.GetECMALineAndUTF16CharacterOfPosition(c.SourceFile, pos)
	return lsproto.Position{Line: uint32(ln), Character: uint32(ch)}
}

// FormatOptions returns the format code settings from the language service.
func (c *Context) FormatOptions() *lsutil.FormatCodeSettings {
	return c.fixCtx.LS.FormatOptions()
}

// FixAction describes a single code action that a fixable wants to produce.
type FixAction struct {
	Description string
	Run         func(tracker *change.Tracker)
}

// NewFixAction creates a tracker, runs the action's edit closure, and returns
// a *ls.CodeAction wrapping the resulting edits for the current SourceFile.
// Returns nil if the closure produced no edits.
func (c *Context) NewFixAction(action FixAction) *ls.CodeAction {
	tracker := change.NewTracker(
		c.ctx,
		c.fixCtx.Program.Options(),
		c.fixCtx.LS.FormatOptions(),
		ls.LanguageService_converters(c.fixCtx.LS),
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
