package compiler

import (
	"github.com/microsoft/typescript-go/internal/compiler"
	"github.com/microsoft/typescript-go/internal/diagnostics"
	"github.com/microsoft/typescript-go/internal/tsoptions"
	"github.com/microsoft/typescript-go/internal/vfs"
)

// NewCompilerHost adapts the rooted-path API. Shared callers provide absolute paths.
func NewCompilerHost(fs vfs.FS, defaultLibraryPath string, cache tsoptions.ExtendedConfigCache, trace func(*diagnostics.Message, ...any), _ any) compiler.CompilerHost {
	return compiler.NewCompilerHost("/", fs, defaultLibraryPath, cache, trace)
}

// The modern program obtains its base directory from the parsed configuration.
// Preserve that behavior with the legacy host-based API.
type rootedCompilerHost struct {
	compiler.CompilerHost
	directory string
}

func (h *rootedCompilerHost) GetCurrentDirectory() string { return h.directory }

func NewProgram(options compiler.ProgramOptions) *compiler.Program {
	if options.Config != nil && options.Config.GetCurrentDirectory() != "" {
		options.Host = &rootedCompilerHost{options.Host, options.Config.GetCurrentDirectory()}
	}
	return compiler.NewProgram(options)
}
