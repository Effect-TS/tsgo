package bundledeffect

import (
	"fmt"
	"io/fs"
	"maps"
	"os"
	pathpkg "path"
	"path/filepath"
	"runtime"
	"sync"
	"testing/fstest"
)

type EffectVersion string

const (
	EffectV3 EffectVersion = "effect-v3"
	EffectV4 EffectVersion = "effect-v4"
)

func EffectTsGoRootPath() string {
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		panic("failed to get caller info for EffectTsGoRootPath")
	}
	return filepath.Dir(filepath.Dir(filepath.Dir(filename)))
}

func PackagePath(version EffectVersion, packageName string) string {
	return filepath.Join(EffectTsGoRootPath(), "testdata", "tests", string(version), "node_modules", filepath.FromSlash(packageName))
}

func EnsurePackageInstalled(version EffectVersion, packageName string) error {
	path := PackagePath(version, packageName)
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return fmt.Errorf("package not installed at %s", path)
	}
	return nil
}

type cacheKey struct {
	version     EffectVersion
	packageName string
}

var (
	fsCacheMu sync.Mutex
	fsCaches  = map[cacheKey]func() map[string]any{}
)

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
			testfs[vfsPath] = &fstest.MapFile{Data: content}
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

func MountEffect(version EffectVersion, testfs map[string]any) error {
	packages := []string{"effect", "pure-rand", "@standard-schema/spec", "fast-check", "@types/node"}
	for _, packageName := range packages {
		if err := EnsurePackageInstalled(version, packageName); err != nil {
			return err
		}
		maps.Copy(testfs, packageFSCache(version, packageName)())
	}

	packageJSONPath := filepath.Join(EffectTsGoRootPath(), "testdata", "tests", string(version), "package.json")
	packageJSON, err := os.ReadFile(packageJSONPath)
	if err != nil {
		return err
	}
	testfs["/.src/package.json"] = &fstest.MapFile{Data: packageJSON}
	return nil
}
