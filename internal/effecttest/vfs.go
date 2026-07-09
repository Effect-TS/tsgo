// Package effecttest provides test utilities for Effect diagnostic tests.
package effecttest

import (
	"runtime"
	"strings"
	"sync"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/compiler"
)

// programSemaphore limits the number of concurrent TypeScript program
// compilations to avoid OOM in memory-constrained environments.
var programSemaphore = make(chan struct{}, maxConcurrentPrograms())

func maxConcurrentPrograms() int {
	n := runtime.GOMAXPROCS(0)
	n = max(n, 1)
	return n
}

// AcquireProgram acquires a slot from the program semaphore.
func AcquireProgram() { programSemaphore <- struct{}{} }

// ReleaseProgram releases a slot back to the program semaphore, clears
// per-program caches to allow GC of the program, and triggers GC.
func ReleaseProgram() {
	<-programSemaphore
	runtime.GC()
}

// astCacheKey combines EffectVersion and filename to avoid cross-version
// collisions (V3 and V4 mount different package contents at the same VFS paths).
type astCacheKey struct {
	version  bundledeffect.EffectVersion
	fileName string
}

// parsedASTCache caches parsed ASTs for node_modules package files.
// These files are identical across all tests of the same EffectVersion,
// so parsing them once and reusing the ASTs across programs saves
// significant memory and CPU. The cache is bounded by the number of
// unique package files per version (finite).
var parsedASTCache sync.Map // map[astCacheKey]*ast.SourceFile

// parsedLibCache caches parsed ASTs for bundled lib files (e.g. lib.es2022.d.ts).
// Lib files are version-independent and identical across all tests, so they
// are keyed by filename only. There are ~108 lib files, so this cache is bounded.
var parsedLibCache sync.Map // map[string]*ast.SourceFile

// cachingCompilerHost wraps a CompilerHost and caches GetSourceFile results
// for files under /node_modules/ and bundled lib files. Test-specific files
// are not cached.
type cachingCompilerHost struct {
	compiler.CompilerHost
	version bundledeffect.EffectVersion
}

func (h *cachingCompilerHost) GetSourceFile(opts ast.SourceFileParseOptions) *ast.SourceFile {
	// Cache bundled lib files (version-independent, keyed by filename only)
	if bundled.IsBundled(opts.FileName) {
		if cached, ok := parsedLibCache.Load(opts.FileName); ok {
			return cached.(*ast.SourceFile)
		}
		sf := h.CompilerHost.GetSourceFile(opts)
		if sf != nil {
			parsedLibCache.Store(opts.FileName, sf)
		}
		return sf
	}
	// Cache package files (under /node_modules/, keyed by version + filename)
	if strings.HasPrefix(opts.FileName, "/node_modules/") {
		key := astCacheKey{version: h.version, fileName: opts.FileName}
		if cached, ok := parsedASTCache.Load(key); ok {
			return cached.(*ast.SourceFile)
		}
		sf := h.CompilerHost.GetSourceFile(opts)
		if sf != nil {
			parsedASTCache.Store(key, sf)
		}
		return sf
	}
	return h.CompilerHost.GetSourceFile(opts)
}
