package rule

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/directives"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// Context bundles the checker, source file, and default severity for a rule invocation.
// It provides a NewDiagnostic helper that simplifies diagnostic creation.
type Context struct {
	Checker         *checker.Checker
	SourceFile      *ast.SourceFile
	defaultSeverity etscore.Severity
}

// NewContext creates a new Context for a rule invocation.
func NewContext(c *checker.Checker, sf *ast.SourceFile, defaultSeverity etscore.Severity) *Context {
	return &Context{
		Checker:         c,
		SourceFile:      sf,
		defaultSeverity: defaultSeverity,
	}
}

// GetErrorRange computes the error range for a node in the context's source file.
// Use this in rules that don't have Analyze functions to get a location before calling NewDiagnostic.
func (ctx *Context) GetErrorRange(node *ast.Node) core.TextRange {
	return scanner.GetErrorRangeForNode(ctx.SourceFile, node)
}

// NewDiagnostic creates a diagnostic using the context's source file and default severity.
// The loc is the pre-computed error range, message provides the diagnostic code and key,
// relatedInformation can be nil, and args are variadic message format arguments.
func (ctx *Context) NewDiagnostic(sf *ast.SourceFile, loc core.TextRange, message *diagnostics.Message, relatedInformation []*ast.Diagnostic, args ...string) *ast.Diagnostic {
	var messageArgs []string
	if len(args) > 0 {
		messageArgs = args
	}
	return ast.NewDiagnosticFromSerialized(
		sf,
		loc,
		message.Code(),
		directives.ToCategory(ctx.defaultSeverity),
		message.Key(),
		messageArgs,
		nil,
		relatedInformation,
		false,
		false,
		false,
	)
}
