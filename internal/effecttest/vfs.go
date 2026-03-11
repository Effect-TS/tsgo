// Package effecttest provides test utilities for Effect diagnostic tests.
package effecttest

import (
	"fmt"
	"io/fs"
	"maps"
	"os"
	pathpkg "path"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing/fstest"

	"github.com/effect-ts/effect-typescript-go/internal/rules"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/compiler"
)

// EffectVersion identifies an Effect major version for test infrastructure.
type EffectVersion string

const (
	// EffectV3 targets the Effect V3 test workspace (testdata/tests/effect-v3).
	EffectV3 EffectVersion = "effect-v3"
	// EffectV4 targets the Effect V4 test workspace (testdata/tests/effect-v4).
	EffectV4 EffectVersion = "effect-v4"
)

// EffectTsGoRootPath returns the path to the effect-typescript-go repo root.
// This is determined relative to this source file's location.
func EffectTsGoRootPath() string {
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		panic("failed to get caller info for EffectTsGoRootPath")
	}
	// This file is at internal/effecttest/vfs.go, so root is ../../
	return filepath.Dir(filepath.Dir(filepath.Dir(filename)))
}

// EffectPackagePath returns the path to the Effect package in node_modules.
func EffectPackagePath(version EffectVersion) string {
	return PackagePath(version, "effect")
}

// PackagePath returns the path to a package in node_modules for the given version.
func PackagePath(version EffectVersion, packageName string) string {
	return filepath.Join(EffectTsGoRootPath(), "testdata", "tests", string(version), "node_modules", filepath.FromSlash(packageName))
}

// EnsureEffectInstalled returns an error if Effect is not installed.
func EnsureEffectInstalled(version EffectVersion) error {
	return EnsurePackageInstalled(version, "effect")
}

// EnsurePackageInstalled returns an error if a package is not installed.
func EnsurePackageInstalled(version EffectVersion, packageName string) error {
	path := PackagePath(version, packageName)
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return fmt.Errorf("package not installed at %s", path)
	}
	return nil
}

// programSemaphore limits the number of concurrent TypeScript program
// compilations to avoid OOM in memory-constrained environments.
var programSemaphore = make(chan struct{}, maxConcurrentPrograms())

func maxConcurrentPrograms() int {
	n := runtime.GOMAXPROCS(0) / 3
	if n < 1 {
		n = 1
	}
	if n > 3 {
		n = 3
	}
	return n
}

// AcquireProgram acquires a slot from the program semaphore.
func AcquireProgram() { programSemaphore <- struct{}{} }

// ReleaseProgram releases a slot back to the program semaphore, clears
// per-program caches to allow GC of the program, and triggers GC.
func ReleaseProgram() {
	typeparser.ClearDiscoverPackagesCache()
	rules.ClearDuplicatePackageCache()
	<-programSemaphore
	runtime.GC()
}

// cacheKey uniquely identifies a package cache entry by version and package name.
type cacheKey struct {
	version     EffectVersion
	packageName string
}

var (
	fsCacheMu sync.Mutex
	fsCaches  = map[cacheKey]func() map[string]any{}
)

// packageFSCache returns a cached loader for a package's files.
// The loader is created once per (version, packageName) combination.
func packageFSCache(version EffectVersion, packageName string) func() map[string]any {
	key := cacheKey{version: version, packageName: packageName}
	fsCacheMu.Lock()
	defer fsCacheMu.Unlock()
	if loader, ok := fsCaches[key]; ok {
		return loader
	}
	loader := sync.OnceValue(func() map[string]any {
		packagePath := PackagePath(version, packageName)
		testfs := make(map[string]any)

		// Walk the entire package directory and add all files.
		packageFS := os.DirFS(packagePath)
		err := fs.WalkDir(packageFS, ".", func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if d.IsDir() {
				return nil
			}
			content, err := fs.ReadFile(packageFS, path)
			if err != nil {
				return err
			}
			vfsPath := pathpkg.Join("/node_modules", packageName, path)
			testfs[vfsPath] = &fstest.MapFile{
				Data: content,
			}
			return nil
		})
		if err != nil {
			panic(fmt.Sprintf("Failed to read package directory: %v", err))
		}

		return testfs
	})
	fsCaches[key] = loader
	return loader
}

// astCacheKey combines EffectVersion and filename to avoid cross-version
// collisions (V3 and V4 mount different package contents at the same VFS paths).
type astCacheKey struct {
	version  EffectVersion
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
	version EffectVersion
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

// MountEffect copies the Effect package files into the provided test filesystem.
// The Effect files are mounted at /node_modules/effect/ in the VFS.
func MountEffect(version EffectVersion, testfs map[string]any) error {
	if err := EnsurePackageInstalled(version, "effect"); err != nil {
		return err
	}
	if err := EnsurePackageInstalled(version, "pure-rand"); err != nil {
		return err
	}
	if err := EnsurePackageInstalled(version, "@standard-schema/spec"); err != nil {
		return err
	}
	if err := EnsurePackageInstalled(version, "fast-check"); err != nil {
		return err
	}
	if err := EnsurePackageInstalled(version, "@types/node"); err != nil {
		return err
	}

	// Copy from cache into the test filesystem
	maps.Copy(testfs, packageFSCache(version, "effect")())
	maps.Copy(testfs, packageFSCache(version, "pure-rand")())
	maps.Copy(testfs, packageFSCache(version, "@standard-schema/spec")())
	maps.Copy(testfs, packageFSCache(version, "fast-check")())
	maps.Copy(testfs, packageFSCache(version, "@types/node")())
	return nil
}
