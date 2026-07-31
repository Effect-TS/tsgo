package bundledeffect

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	_ "embed"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"maps"
	pathpkg "path"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
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
	if len(packageFSCache(version, packageName)()) == 0 {
		return fmt.Errorf("package not installed at %s", PackagePath(version, packageName))
	}
	return nil
}

func PackageFile(version EffectVersion, packageName string, file string) (string, bool) {
	path := pathpkg.Join(string(version), "node_modules", packageName, file)
	content, ok := fixtures().files[path]
	return string(content), ok
}

type fixtureProfileManifest struct {
	Requested map[string]string `json:"requested"`
	Resolved  map[string]string `json:"resolved"`
}

type fixtureManifest struct {
	SchemaVersion int                               `json:"schemaVersion"`
	Profiles      map[string]fixtureProfileManifest `json:"profiles"`
	TreeSHA256    string                            `json:"treeSha256"`
}

type fixtureBundle struct {
	files    map[string][]byte
	manifest fixtureManifest
}

//go:embed testfixtures.tar.gz
var fixtureArchive []byte

var fixtures = sync.OnceValue(func() fixtureBundle {
	gzipReader, err := gzip.NewReader(bytes.NewReader(fixtureArchive))
	if err != nil {
		panic(fmt.Sprintf("Failed to open embedded fixtures: %v", err))
	}
	defer gzipReader.Close()

	files := make(map[string][]byte)
	var manifest fixtureManifest
	tarReader := tar.NewReader(gzipReader)
	for {
		header, err := tarReader.Next()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			panic(fmt.Sprintf("Failed to read embedded fixtures: %v", err))
		}
		if !header.FileInfo().Mode().IsRegular() {
			continue
		}
		content, err := io.ReadAll(tarReader)
		if err != nil {
			panic(fmt.Sprintf("Failed to read embedded fixture %s: %v", header.Name, err))
		}
		name := pathpkg.Clean(header.Name)
		if name == "manifest.json" {
			if err := json.Unmarshal(content, &manifest); err != nil {
				panic(fmt.Sprintf("Failed to decode embedded fixture manifest: %v", err))
			}
			continue
		}
		if _, exists := files[name]; exists {
			panic("Duplicate embedded fixture: " + name)
		}
		files[name] = content
	}
	if manifest.SchemaVersion != 1 {
		panic(fmt.Sprintf("Unsupported embedded fixture schema version: %d", manifest.SchemaVersion))
	}
	if digest := fixtureTreeSHA256(files); digest != manifest.TreeSHA256 {
		panic(fmt.Sprintf("Embedded fixture digest mismatch: expected %s, got %s", manifest.TreeSHA256, digest))
	}
	return fixtureBundle{files: files, manifest: manifest}
})

func fixtureTreeSHA256(files map[string][]byte) string {
	paths := slices.Sorted(maps.Keys(files))
	hash := sha256.New()
	for _, path := range paths {
		hash.Write([]byte(path))
		hash.Write([]byte{0})
		contentHash := sha256.Sum256(files[path])
		hash.Write(contentHash[:])
	}
	return hex.EncodeToString(hash.Sum(nil))
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
		prefix := pathpkg.Join(string(version), "node_modules", packageName) + "/"
		testfs := make(map[string]any)
		for path, content := range fixtures().files {
			relativePath, ok := strings.CutPrefix(path, prefix)
			if !ok {
				continue
			}
			vfsPath := pathpkg.Join("/node_modules", packageName, relativePath)
			testfs[vfsPath] = &fstest.MapFile{Data: content}
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

	packageJSONPath := pathpkg.Join(string(version), "package.json")
	packageJSON, ok := fixtures().files[packageJSONPath]
	if !ok {
		return fmt.Errorf("package.json not installed at %s", packageJSONPath)
	}
	testfs["/.src/package.json"] = &fstest.MapFile{Data: packageJSON}
	return nil
}

func MountVitest(version EffectVersion, testfs map[string]any) error {
	packages := []string{"vitest", "@vitest/runner", "@effect/vitest"}
	for _, packageName := range packages {
		if err := EnsurePackageInstalled(version, packageName); err != nil {
			return err
		}
		maps.Copy(testfs, packageFSCache(version, packageName)())
	}
	return nil
}
