package effecttest

import (
	"slices"
	"sync"
	"testing/fstest"
	"time"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

// sharedPackages is a read-only VFS of the packages MountEffect (and
// MountVitest) mount, built once per version and shared by every test, so a
// test does not copy, sort and re-index thousands of package files.
type sharedPackages struct {
	fs vfs.FS
	// contents holds the file contents as strings, so reads do not copy them.
	contents    map[string]string
	packageJSON []byte
}

type sharedPackagesKey struct {
	version bundledeffect.EffectVersion
	vitest  bool
}

var sharedPackagesCache sync.Map // map[sharedPackagesKey]func() (*sharedPackages, error)

func getSharedPackages(version bundledeffect.EffectVersion, vitest bool) (*sharedPackages, error) {
	key := sharedPackagesKey{version: version, vitest: vitest}
	load, _ := sharedPackagesCache.LoadOrStore(key, sync.OnceValues(func() (*sharedPackages, error) {
		files := make(map[string]any)
		if err := bundledeffect.MountEffect(version, files); err != nil {
			return nil, err
		}
		if vitest {
			if err := bundledeffect.MountVitest(version, files); err != nil {
				return nil, err
			}
		}
		// The project's package.json lives in the test's own directory.
		packageJSON := files["/.src/package.json"].(*fstest.MapFile).Data
		delete(files, "/.src/package.json")
		contents := make(map[string]string, len(files))
		for name, file := range files {
			contents[name] = string(file.(*fstest.MapFile).Data)
		}
		return &sharedPackages{
			fs:          vfstest.FromMap(files, tspath.CaseSensitive),
			contents:    contents,
			packageJSON: packageJSON,
		}, nil
	}))
	return load.(func() (*sharedPackages, error))()
}

// overlayFS layers a test's own files over the shared packages. The test's
// files win; directory listings merge both layers, since tests add their own
// packages (and files) under /node_modules. Writes go to the test's layer.
type overlayFS struct {
	upper  vfs.FS
	shared *sharedPackages
}

func newOverlayFS(upper vfs.FS, shared *sharedPackages) vfs.FS {
	return &overlayFS{upper: upper, shared: shared}
}

func (o *overlayFS) CaseSensitivity() tspath.CaseSensitivity { return o.upper.CaseSensitivity() }

func (o *overlayFS) FileExists(path tspath.RootedFilePath) bool {
	if o.upper.FileExists(path) {
		return true
	}
	_, ok := o.shared.contents[string(path)]
	return ok
}

func (o *overlayFS) ReadFile(path tspath.RootedFilePath) (string, bool) {
	if contents, ok := o.upper.ReadFile(path); ok {
		return contents, true
	}
	contents, ok := o.shared.contents[string(path)]
	return contents, ok
}

func (o *overlayFS) WriteFile(path tspath.RootedFilePath, data string) error {
	return o.upper.WriteFile(path, data)
}

func (o *overlayFS) AppendFile(path tspath.RootedFilePath, data string) error {
	return o.upper.AppendFile(path, data)
}

func (o *overlayFS) Remove(path tspath.RootedPath) error { return o.upper.Remove(path) }

func (o *overlayFS) Chtimes(path tspath.RootedPath, aTime time.Time, mTime time.Time) error {
	return o.upper.Chtimes(path, aTime, mTime)
}

func (o *overlayFS) DirectoryExists(path tspath.RootedDirectoryPath) bool {
	return o.upper.DirectoryExists(path) || o.shared.fs.DirectoryExists(path)
}

func (o *overlayFS) GetAccessibleEntries(path tspath.RootedDirectoryPath) vfs.Entries {
	if !o.shared.fs.DirectoryExists(path) {
		return o.upper.GetAccessibleEntries(path)
	}
	shared := o.shared.fs.GetAccessibleEntries(path)
	if !o.upper.DirectoryExists(path) {
		return shared
	}
	upper := o.upper.GetAccessibleEntries(path)
	merged := vfs.Entries{
		Files:       mergeNames(upper.Files, shared.Files),
		Directories: mergeNames(upper.Directories, shared.Directories),
	}
	if upper.Symlinks != nil || shared.Symlinks != nil {
		merged.Symlinks = make(map[string]struct{}, len(upper.Symlinks)+len(shared.Symlinks))
		for name := range upper.Symlinks {
			merged.Symlinks[name] = struct{}{}
		}
		for name := range shared.Symlinks {
			merged.Symlinks[name] = struct{}{}
		}
	}
	return merged
}

func mergeNames(a, b []string) []string {
	merged := slices.Concat(a, b)
	slices.Sort(merged)
	return slices.Compact(merged)
}

func (o *overlayFS) Stat(path tspath.RootedPath) vfs.FileInfo {
	if info := o.upper.Stat(path); info != nil {
		return info
	}
	return o.shared.fs.Stat(path)
}

func (o *overlayFS) Realpath(path tspath.RootedPath) tspath.RootedPath {
	if o.upper.Stat(path) != nil {
		return o.upper.Realpath(path)
	}
	return o.shared.fs.Realpath(path)
}
