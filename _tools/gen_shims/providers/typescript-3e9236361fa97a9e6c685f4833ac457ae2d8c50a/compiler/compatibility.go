package compiler

import (
	"context"

	"github.com/microsoft/typescript-go/internal/compiler"
)

// NewProgram adapts the context-aware API. The beta program cannot be
// cancelled while it is built, so it never returns an error.
func NewProgram(_ context.Context, options compiler.ProgramOptions) (*compiler.Program, error) {
	return compiler.NewProgram(options), nil
}
