package fixable

import (
	"context"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// Context bundles the code-fix request data and provides helpers for fixable implementations.
// It replaces the (context.Context, *rewriter.Tracker, *ls.CodeFixContext) parameter triple,
// giving each fixable self-contained access to the checker, tracker lifecycle, and edit finalization.
type Context struct {
	SourceFile *ast.SourceFile
	Span       core.TextRange
	ErrorCode  int32
	Options    *etscore.ResolvedEffectPluginOptions
	Program    *compiler.Program
	Checker    *checker.Checker
	TypeParser *typeparser.TypeParser

	Context context.Context
	fixCtx  *ls.CodeFixContext
}

// NewContext creates a fixable Context from the standard code-fix request parameters.
func NewContext(ctx context.Context, fixCtx *ls.CodeFixContext, options *etscore.ResolvedEffectPluginOptions, checker *checker.Checker, tp *typeparser.TypeParser) *Context {
	if fixCtx == nil || fixCtx.Program == nil {
		panic("fixable.NewContext: nil program")
	}
	if checker == nil {
		panic("fixable.NewContext: nil checker")
	}
	return &Context{
		SourceFile: fixCtx.SourceFile,
		Span:       fixCtx.Span,
		ErrorCode:  fixCtx.ErrorCode,
		Options:    options,
		Program:    fixCtx.Program,
		Checker:    checker,
		TypeParser: tp,
		Context:    ctx,
		fixCtx:     fixCtx,
	}
}

// BytePosToLSPPosition converts a single byte offset in the context's SourceFile
// to an lsproto.Position using ECMA line/character position.
func (c *Context) BytePosToLSPPosition(pos int) lsproto.Position {
	ln, ch := scanner.GetECMALineAndUTF16CharacterOfPosition(c.SourceFile, pos)
	return lsproto.Position{Line: uint32(ln), Character: uint32(ch)}
}

// FixAction describes a single code action that a fixable wants to produce.
type FixAction struct {
	Description string
	Run         func(tracker *rewriter.Tracker)
}

// NewFixAction creates a tracker, runs the action's edit closure, and returns
// a *ls.CodeAction wrapping the resulting edits for the current SourceFile.
// Returns nil if the closure produced no edits.
func (c *Context) NewFixAction(action FixAction) *ls.CodeAction {
	rawTracker := change.NewTracker(
		c.Context,
		c.Program.Options(),
		c.fixCtx.LS.FormatOptions(),
		ls.LanguageService_converters(c.fixCtx.LS),
	)
	tracker := rewriter.NewTracker(rawTracker)
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
