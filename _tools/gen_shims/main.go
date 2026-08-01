package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"go/parser"
	"go/token"
	"go/types"
	"io"
	"io/fs"
	"log"
	"maps"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"strings"

	"golang.org/x/mod/modfile"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
	"golang.org/x/tools/go/packages"
)

const tsgoInternalPrefix = "github.com/microsoft/typescript-go/internal/"

const generatedMarker = ".generated-by-gen-shims"

const shimModulePrefix = "github.com/microsoft/typescript-go/shim"

var packagesToShim = []string{
	"api",
	"ast",
	"astnav",
	"bundled",
	"checker",
	"collections",
	"compiler",
	"core",
	"diagnostics",
	"evaluator",
	"execute/tsc",
	"format",
	"fourslash",
	"jsnum",
	"locale",
	"ls",
	"ls/autoimport",
	"ls/change",
	"ls/lsconv",
	"ls/lsutil",
	"lsp",
	"lsp/lsproto",
	"module",
	"modulespecifiers",
	"packagejson",
	"parser",
	"project",
	"project/logging",
	"repo",
	"scanner",
	"sourcemap",
	"testrunner",
	"testutil",
	"testutil/lsptestutil",
	"testutil/baseline",
	"testutil/harnessutil",
	"testutil/tsbaseline",
	"tsoptions",
	"tspath",
	"vfs",
	"vfs/cachedvfs",
	"vfs/iovfs",
	"vfs/vfsmatch",
	"vfs/osvfs",
}

type stringFlags []string

func (f *stringFlags) String() string { return strings.Join(*f, ",") }

func (f *stringFlags) Set(value string) error {
	*f = append(*f, value)
	return nil
}

func signatureHasUnexportedType(t types.Signature) bool {
	if params := t.Params(); params != nil {
		for i := range params.Len() {
			ty := params.At(i).Type()

			if ptrType, ok := ty.(*types.Pointer); ok {
				ty = ptrType.Elem()
			}
			if named, ok := ty.(*types.Named); ok {
				if !named.Obj().Exported() {
					return true
				}
			}
		}
	}
	return false
}

type ExtraShim struct {
	ExtraFunctions  []string            `json:"ExtraFunctions,omitempty"`
	ExtraMethods    map[string][]string `json:"ExtraMethods,omitempty"`
	ExtraFields     map[string][]string `json:"ExtraFields,omitempty"`
	CompactFields   map[string][]string `json:"CompactFields,omitempty"`
	IgnoreFunctions []string            `json:"IgnoreFunctions,omitempty"`
}

type shimInputs struct {
	extra   map[string]ExtraShim
	helpers map[string][]byte
}

func normalizeStrings(values []string) []string {
	result := slices.Compact(slices.Sorted(slices.Values(values)))
	if len(result) == 0 {
		return nil
	}
	return result
}

func mergeStringMap(base map[string][]string, overlay map[string][]string) map[string][]string {
	if len(base) == 0 && len(overlay) == 0 {
		return nil
	}
	result := make(map[string][]string, len(base)+len(overlay))
	for name, values := range base {
		result[name] = normalizeStrings(values)
	}
	for name, values := range overlay {
		result[name] = normalizeStrings(slices.Concat(result[name], values))
	}
	return result
}

func mergeExtraShim(base, overlay ExtraShim) ExtraShim {
	return ExtraShim{
		ExtraFunctions:  normalizeStrings(slices.Concat(base.ExtraFunctions, overlay.ExtraFunctions)),
		ExtraMethods:    mergeStringMap(base.ExtraMethods, overlay.ExtraMethods),
		ExtraFields:     mergeStringMap(base.ExtraFields, overlay.ExtraFields),
		CompactFields:   mergeStringMap(base.CompactFields, overlay.CompactFields),
		IgnoreFunctions: normalizeStrings(slices.Concat(base.IgnoreFunctions, overlay.IgnoreFunctions)),
	}
}

func parseExtraShim(data []byte, source string) (ExtraShim, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var extra ExtraShim
	if err := decoder.Decode(&extra); err != nil {
		return ExtraShim{}, fmt.Errorf("parse %s: %w", source, err)
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		if err == nil {
			return ExtraShim{}, fmt.Errorf("parse %s: unexpected trailing JSON value", source)
		}
		return ExtraShim{}, fmt.Errorf("parse %s: %w", source, err)
	}
	return extra, nil
}

func knownPackagePaths() map[string]bool {
	known := make(map[string]bool, len(packagesToShim)+1)
	for _, packagePath := range packagesToShim {
		known[packagePath] = true
	}
	known["vfs/vfstest"] = true
	return known
}

func loadShimInputs(configRoot string, overlayRoots []string) (shimInputs, error) {
	inputs := shimInputs{extra: map[string]ExtraShim{}, helpers: map[string][]byte{}}
	known := knownPackagePaths()
	roots := append([]string{configRoot}, overlayRoots...)
	for rootIndex, root := range roots {
		info, err := os.Stat(root)
		if err != nil {
			return shimInputs{}, fmt.Errorf("read shim input root %s: %w", root, err)
		}
		if !info.IsDir() {
			return shimInputs{}, fmt.Errorf("shim input root %s is not a directory", root)
		}
		err = filepath.WalkDir(root, func(filePath string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if entry.IsDir() {
				return nil
			}
			relative, err := filepath.Rel(root, filePath)
			if err != nil {
				return err
			}
			packagePath := filepath.ToSlash(filepath.Dir(relative))
			name := entry.Name()
			isExtra := name == "extra-shim.json"
			isHelper := filepath.Ext(name) == ".go" && (rootIndex == 0 || name != "shim.go")
			if !isExtra && !isHelper {
				return nil
			}
			if !known[packagePath] {
				return fmt.Errorf("shim input %s targets unknown package path %q", filePath, packagePath)
			}
			data, err := os.ReadFile(filePath)
			if err != nil {
				return err
			}
			if isExtra {
				extra, err := parseExtraShim(data, filePath)
				if err != nil {
					return err
				}
				if rootIndex == 0 {
					inputs.extra[packagePath] = extra
				} else {
					inputs.extra[packagePath] = mergeExtraShim(inputs.extra[packagePath], extra)
				}
				return nil
			}
			if _, err := parser.ParseFile(token.NewFileSet(), filePath, data, parser.AllErrors); err != nil {
				return fmt.Errorf("validate shim helper %s: %w", filePath, err)
			}
			relative = filepath.Clean(relative)
			if existing, ok := inputs.helpers[relative]; ok && !bytes.Equal(existing, data) {
				return fmt.Errorf("conflicting handwritten shim helper %s", relative)
			}
			inputs.helpers[relative] = data
			return nil
		})
		if err != nil {
			return shimInputs{}, fmt.Errorf("load shim inputs from %s: %w", root, err)
		}
	}
	return inputs, nil
}

func modulePaths() []string {
	modules := make([]string, 0, len(packagesToShim))
	for _, packagePath := range packagesToShim {
		if packagePath != "vfs/vfsmatch" {
			modules = append(modules, packagePath)
		}
	}
	modules = append(modules, "vfs/vfstest")
	slices.Sort(modules)
	return modules
}

func isShimWorkPath(repositoryRoot, path string) (bool, error) {
	resolved := filepath.FromSlash(path)
	if !filepath.IsAbs(resolved) {
		resolved = filepath.Join(repositoryRoot, resolved)
	}
	relative, err := filepath.Rel(filepath.Join(repositoryRoot, "shim"), filepath.Clean(resolved))
	if err != nil {
		return false, err
	}
	return relative == "." || relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator)) && !filepath.IsAbs(relative), nil
}

func updateGoWork(repositoryRoot string, content []byte, modules []string) ([]byte, error) {
	workFile, err := modfile.ParseWork("go.work", content, nil)
	if err != nil {
		return nil, fmt.Errorf("parse go.work: %w", err)
	}
	for _, use := range slices.Clone(workFile.Use) {
		isShim, err := isShimWorkPath(repositoryRoot, use.Path)
		if err != nil {
			return nil, fmt.Errorf("resolve go.work use %s: %w", use.Path, err)
		}
		if isShim {
			if err := workFile.DropUse(use.Path); err != nil {
				return nil, fmt.Errorf("drop go.work use %s: %w", use.Path, err)
			}
		}
	}
	for _, replacement := range slices.Clone(workFile.Replace) {
		isShim, err := isShimWorkPath(repositoryRoot, replacement.New.Path)
		if err != nil {
			return nil, fmt.Errorf("resolve go.work replacement %s: %w", replacement.New.Path, err)
		}
		if isShim || replacement.Old.Path == shimModulePrefix || strings.HasPrefix(replacement.Old.Path, shimModulePrefix+"/") {
			if err := workFile.DropReplace(replacement.Old.Path, replacement.Old.Version); err != nil {
				return nil, fmt.Errorf("drop go.work replacement for %s: %w", replacement.Old.Path, err)
			}
		}
	}
	modules = slices.Clone(modules)
	slices.Sort(modules)
	for _, module := range modules {
		if err := workFile.AddUse("./shim/"+filepath.ToSlash(module), ""); err != nil {
			return nil, fmt.Errorf("add go.work use for %s: %w", module, err)
		}
	}
	workFile.Cleanup()
	return modfile.Format(workFile.Syntax), nil
}

func writeFile(filePath string, data []byte, mode fs.FileMode) error {
	if err := os.MkdirAll(filepath.Dir(filePath), 0o755); err != nil {
		return err
	}
	file, err := os.OpenFile(filePath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, mode)
	if err != nil {
		return err
	}
	if _, err := file.Write(data); err != nil {
		_ = file.Close()
		return err
	}
	return file.Close()
}

func writeShimHelpers(root string, helpers map[string][]byte) error {
	for relative, data := range helpers {
		if err := writeFile(filepath.Join(root, relative), data, 0o644); err != nil {
			return fmt.Errorf("write shim helper %s: %w", relative, err)
		}
	}
	return nil
}

func preparePackageLoad(repositoryRoot string) ([]string, string, func(), error) {
	temporaryRoot, err := os.MkdirTemp(repositoryRoot, ".gen-shims-load-")
	if err != nil {
		return nil, "", nil, err
	}
	cleanup := func() { _ = os.RemoveAll(temporaryRoot) }
	etscoreRoot := filepath.Join(temporaryRoot, "etscore")
	etscoreModule := []byte("module github.com/effect-ts/tsgo/etscore\n\ngo 1.26\n")
	etscoreSource := []byte(`package etscore

type EffectPluginOptions struct{}

func ParseFromPlugins(any) *EffectPluginOptions { return nil }

func EnterCommandLineMode() func() { return func() {} }
`)
	if err := writeFile(filepath.Join(etscoreRoot, "go.mod"), etscoreModule, 0o644); err != nil {
		cleanup()
		return nil, "", nil, err
	}
	if err := writeFile(filepath.Join(etscoreRoot, "etscore.go"), etscoreSource, 0o644); err != nil {
		cleanup()
		return nil, "", nil, err
	}
	typescriptGoMod, err := os.ReadFile(filepath.Join(repositoryRoot, "typescript-go", "go.mod"))
	if err != nil {
		cleanup()
		return nil, "", nil, err
	}
	modFile := filepath.Join(temporaryRoot, "typescript-go.mod")
	typescriptGoMod = append(typescriptGoMod, fmt.Sprintf(
		"\nrequire github.com/effect-ts/tsgo/etscore v0.0.0\nreplace github.com/effect-ts/tsgo/etscore => %s\n",
		strconv.Quote(etscoreRoot),
	)...)
	if err := writeFile(modFile, typescriptGoMod, 0o644); err != nil {
		cleanup()
		return nil, "", nil, err
	}
	typescriptGoSum, err := os.ReadFile(filepath.Join(repositoryRoot, "typescript-go", "go.sum"))
	if err != nil {
		cleanup()
		return nil, "", nil, err
	}
	if err := writeFile(strings.TrimSuffix(modFile, ".mod")+".sum", typescriptGoSum, 0o644); err != nil {
		cleanup()
		return nil, "", nil, err
	}
	environment := slices.DeleteFunc(os.Environ(), func(value string) bool {
		return strings.HasPrefix(value, "GOWORK=") || strings.HasPrefix(value, "GOFLAGS=") || strings.HasPrefix(value, "GO111MODULE=")
	})
	environment = append(environment, "GOWORK=off", "GO111MODULE=on")
	return environment, modFile, cleanup, nil
}

func resetOutputRoot(root string) error {
	if err := os.RemoveAll(root); err != nil {
		return err
	}
	return os.MkdirAll(root, 0o755)
}

func main() {
	if err := run(); err != nil {
		log.Fatal(err)
	}
}

func run() error {
	var extraShimRoots stringFlags
	var repositoryRoot string
	flag.StringVar(&repositoryRoot, "repository-root", "", "path to the repository root")
	flag.Var(&extraShimRoots, "extra-shim-root", "additional shim config/helper root (repeatable)")
	flag.Parse()

	workingDirectory, err := os.Getwd()
	if err != nil {
		return err
	}
	if repositoryRoot == "" {
		repositoryRoot = workingDirectory
		if filepath.Base(workingDirectory) == "gen_shims" && filepath.Base(filepath.Dir(workingDirectory)) == "_tools" {
			repositoryRoot = filepath.Join(workingDirectory, "..", "..")
		}
	}
	repositoryRoot, err = filepath.Abs(repositoryRoot)
	if err != nil {
		return fmt.Errorf("resolve repository root: %w", err)
	}
	shimPath := filepath.Join(repositoryRoot, "shim")
	goWorkPath := filepath.Join(repositoryRoot, "go.work")
	configRoot := filepath.Join(repositoryRoot, "_tools", "gen_shims", "config")
	for index, root := range extraShimRoots {
		if !filepath.IsAbs(root) {
			extraShimRoots[index] = filepath.Join(repositoryRoot, root)
		}
	}
	inputs, err := loadShimInputs(configRoot, extraShimRoots)
	if err != nil {
		return err
	}
	goWork, err := os.ReadFile(goWorkPath)
	if err != nil {
		return err
	}
	updatedGoWork, err := updateGoWork(repositoryRoot, goWork, modulePaths())
	if err != nil {
		return err
	}

	packagesToShimFullNames := make([]string, len(packagesToShim))
	for i, pkg := range packagesToShim {
		packagesToShimFullNames[i] = tsgoInternalPrefix + pkg
	}

	environment, modFile, cleanupPackageLoad, err := preparePackageLoad(repositoryRoot)
	if err != nil {
		return fmt.Errorf("prepare TypeScript-Go package loading: %w", err)
	}
	defer cleanupPackageLoad()
	loadedPackages, err := packages.Load(&packages.Config{
		Dir:        filepath.Join(repositoryRoot, "typescript-go"),
		Env:        environment,
		BuildFlags: []string{"-modfile=" + modFile},
		Mode:       packages.LoadSyntax,
	}, packagesToShimFullNames...)
	if err != nil {
		return fmt.Errorf("load TypeScript-Go packages: %w", err)
	}
	if packages.PrintErrors(loadedPackages) > 0 {
		return errors.New("TypeScript-Go package loading failed")
	}

	if err := resetOutputRoot(shimPath); err != nil {
		return fmt.Errorf("reset shim output: %w", err)
	}
	if err := writeShimHelpers(shimPath, inputs.helpers); err != nil {
		return err
	}

	var shimHeaderBuilder strings.Builder
	var shimBuilder strings.Builder
	var tempBuffer bytes.Buffer

	for _, pkg := range loadedPackages {
		packagePath := strings.TrimPrefix(pkg.PkgPath, tsgoInternalPrefix)
		shimDirPath := filepath.Join(shimPath, filepath.FromSlash(packagePath))
		extraShim := inputs.extra[packagePath]
		if extraShim.ExtraMethods == nil {
			extraShim.ExtraMethods = map[string][]string{}
		}
		if extraShim.ExtraFunctions == nil {
			extraShim.ExtraFunctions = []string{}
		}
		if extraShim.ExtraFields == nil {
			extraShim.ExtraFields = map[string]([]string){}
		}
		if extraShim.CompactFields == nil {
			extraShim.CompactFields = map[string]([]string){}
		}
		if extraShim.IgnoreFunctions == nil {
			extraShim.IgnoreFunctions = []string{}
		}

		// true if directly used, false otherwise
		importedPackages := map[string]bool{}

		importPackage := func(pkg string, directly bool) {
			if directly {
				importedPackages[pkg] = true
			} else if _, ok := importedPackages[pkg]; !ok {
				importedPackages[pkg] = false
			}
		}

		var qualifierOnlyPackageName types.Qualifier = func(p *types.Package) string {
			importPackage(p.Path(), true)
			return p.Name()
		}
		var qualifierEmptyPackageName types.Qualifier = func(p *types.Package) string {
			return ""
		}
		var fieldTypeString func(types.Type) string
		fieldTypeString = func(t types.Type) string {
			switch ty := t.(type) {
			case *types.Named:
				if !ty.Obj().Exported() {
					return fieldTypeString(ty.Underlying())
				}
			case *types.Pointer:
				return "*" + fieldTypeString(ty.Elem())
			case *types.Slice:
				return "[]" + fieldTypeString(ty.Elem())
			case *types.Array:
				return fmt.Sprintf("[%d]%s", ty.Len(), fieldTypeString(ty.Elem()))
			case *types.Map:
				return "map[" + fieldTypeString(ty.Key()) + "]" + fieldTypeString(ty.Elem())
			}

			return types.TypeString(t, qualifierOnlyPackageName)
		}

		emitGoLinknameDirective := func(localName string, fn *types.Func) {
			// //go:linkname only allowed in Go files that import "unsafe"
			importPackage("unsafe", false)
			importPackage(pkg.Types.Path(), false)
			shimBuilder.WriteString("//go:linkname ")
			shimBuilder.WriteString(localName)
			shimBuilder.WriteByte(' ')
			shimBuilder.WriteString(fn.Pkg().Path())
			shimBuilder.WriteByte('.')
			if recv := fn.Signature().Recv(); recv != nil {
				shimBuilder.WriteByte('(')
				shimBuilder.WriteString(types.TypeString(recv.Type(), qualifierEmptyPackageName))
				shimBuilder.WriteByte(')')
				shimBuilder.WriteByte('.')
			}
			shimBuilder.WriteString(fn.Name())
			shimBuilder.WriteByte('\n')
		}

		emitLinkedFunction := func(fn *types.Func) bool {
			if fn.Signature().TypeParams() != nil {
				// https://github.com/golang/go/issues/60425
				// linking to functions with generics is not supported in go:linkname
				return false
			}
			if signatureHasUnexportedType(*fn.Signature()) {
				fmt.Fprintf(os.Stderr, "Skipping %s.%s: references unexported types\n", fn.Pkg().Name(), fn.Name())
				return false
			}
			name := cases.Title(language.English, cases.NoLower).String(fn.Name())
			emitGoLinknameDirective(name, fn)
			shimBuilder.WriteString("func ")
			shimBuilder.WriteString(name)
			types.WriteSignature(&tempBuffer, fn.Signature(), qualifierOnlyPackageName)
			shimBuilder.Write(tempBuffer.Bytes())
			tempBuffer.Reset()
			shimBuilder.WriteString("\n")
			return true
		}

		matchedExtraFunctions := make(map[string]bool, len(extraShim.ExtraFunctions))
		for _, name := range extraShim.ExtraFunctions {
			matchedExtraFunctions[name] = false
		}
		matchedExtraMethods := make(map[string](map[string]bool), len(extraShim.ExtraMethods))
		for name, methods := range extraShim.ExtraMethods {
			matchedExtraMethods[name] = make(map[string]bool, len(methods))
			for _, method := range methods {
				matchedExtraMethods[name][method] = false
			}
		}
		matchedExtraFields := make(map[string]bool, len(extraShim.ExtraFields))
		for name := range extraShim.ExtraFields {
			matchedExtraFields[name] = false
		}
		compactFieldNames := make(map[string]map[string]bool, len(extraShim.CompactFields))
		for name, fields := range extraShim.CompactFields {
			compactFieldNames[name] = make(map[string]bool, len(fields))
			for _, field := range fields {
				compactFieldNames[name][field] = true
			}
		}

		scope := pkg.Types.Scope()
		for _, name := range scope.Names() {
			object := scope.Lookup(name)
			if !object.Exported() {
				fn, isFunc := object.(*types.Func)
				if _, exists := matchedExtraFunctions[name]; isFunc && exists {
					if emitLinkedFunction(fn) {
						matchedExtraFunctions[name] = true
					}
				}
				continue
			}

			printReexport := func(kind string) {
				importPackage(pkg.Types.Path(), true)
				shimBuilder.WriteString(kind)
				shimBuilder.WriteString(" ")
				shimBuilder.WriteString(name)
				shimBuilder.WriteString(" = ")
				shimBuilder.WriteString(pkg.Name)
				shimBuilder.WriteString(".")
				shimBuilder.WriteString(name)
				shimBuilder.WriteString("\n")
			}

			switch object.(type) {
			case *types.TypeName:
				typeName := object.(*types.TypeName)
				t := typeName.Type()
				named, isNamed := t.(*types.Named)
				if isNamed {
					_, nameWithTypeParams, _ := strings.Cut(types.TypeString(named, qualifierOnlyPackageName), ".")
					importPackage(pkg.Types.Path(), true)
					shimBuilder.WriteString("type ")
					shimBuilder.WriteString(nameWithTypeParams)
					shimBuilder.WriteString(" = ")
					shimBuilder.WriteString(pkg.Name)
					shimBuilder.WriteString(".")
					shimBuilder.WriteString(name)

					typeParams := slices.Collect(named.TypeParams().TypeParams())
					if len(typeParams) > 0 {
						// (*typeWriter)typeList
						shimBuilder.WriteByte('[')
						for i, ty := range typeParams {
							if i > 0 {
								shimBuilder.WriteByte(',')
							}
							shimBuilder.WriteString(ty.String())
						}
						shimBuilder.WriteByte(']')
					}

					shimBuilder.WriteString("\n")
				} else {
					printReexport("type")
				}

				if extraMethods, ok := matchedExtraMethods[name]; isNamed && ok {
					for method := range named.Methods() {
						methodName := method.Name()
						if _, exists := extraMethods[methodName]; !exists {
							continue
						}
						extraMethods[methodName] = true
						prefix := name + "_"
						emitGoLinknameDirective(prefix+methodName, method)
						funcDeclStr := types.ObjectString(method, qualifierOnlyPackageName)
						recvStart := 0
						recvEnd := 0
						paramsStart := 0
						for i, s := range funcDeclStr {
							if s == '(' {
								if recvStart == 0 {
									recvStart = i + 1
								}
								if recvEnd != 0 {
									paramsStart = i + 1
									break
								}
							}
							if s == ')' && recvEnd == 0 {
								recvEnd = i
							}
						}
						shimBuilder.WriteString("func ")
						shimBuilder.WriteString(prefix)
						shimBuilder.WriteString(funcDeclStr[recvEnd+2 : paramsStart])
						shimBuilder.WriteString("recv ")
						shimBuilder.WriteString(funcDeclStr[recvStart:recvEnd])
						if method.Signature().Params() != nil {
							shimBuilder.WriteString(", ")
						}
						shimBuilder.WriteString(funcDeclStr[paramsStart:])
						shimBuilder.WriteString("\n")
					}
				}

				if _, ok := matchedExtraFields[name]; isNamed && ok {
					importPackage("unsafe", true)

					matchedExtraFields[name] = true

					var emitExtraStruct func(name string, s *types.Struct)
					emitExtraStruct = func(name string, s *types.Struct) {
						shimBuilder.WriteString("type extra_")
						shimBuilder.WriteString(name)
						shimBuilder.WriteString(" struct {")

						dependencies := [](struct {
							string
							*types.Struct
						}){}
						for field := range s.Fields() {
							shimBuilder.WriteString("\n  ")
							if !field.Embedded() {
								shimBuilder.WriteString(field.Name())
								shimBuilder.WriteByte(' ')
							}

							ptrType, ok := field.Type().(*types.Pointer)
							if ok {
								named, ok := ptrType.Elem().(*types.Named)
								if ok && !named.Obj().Exported() {
									strct, ok := named.Underlying().(*types.Struct)
									if ok {
										n := named.Obj().Name()
										dependencies = append(dependencies, struct {
											string
											*types.Struct
										}{n, strct})
										shimBuilder.WriteString("extra_")
										shimBuilder.WriteString(n)
										continue
									}
								}
							}

							shimBuilder.WriteString(fieldTypeString(field.Type()))
						}
						shimBuilder.WriteString("\n}\n")

						for _, dep := range dependencies {
							emitExtraStruct(dep.string, dep.Struct)
						}
					}

					strct, ok := named.Underlying().(*types.Struct)
					if !ok {
						return fmt.Errorf("expected %v to be struct", name)
					}

					mappedFieldTypes := make(map[string]*types.Var, strct.NumFields())
					mappedFieldIndexes := make(map[string]int, strct.NumFields())
					for i := range strct.NumFields() {
						field := strct.Field(i)
						mappedFieldTypes[field.Name()] = field
						mappedFieldIndexes[field.Name()] = i
					}

					needsFullMirror := false
					for _, field := range extraShim.ExtraFields[name] {
						idx, ok := mappedFieldIndexes[field]
						if !ok {
							return fmt.Errorf("expected struct %q to contain field %q", name, field)
						}
						if idx != 0 || !compactFieldNames[name][field] {
							needsFullMirror = true
							break
						}
					}

					mirrorStructName := "extra_" + name
					if needsFullMirror {
						emitExtraStruct(name, strct)
					}

					for _, field := range extraShim.ExtraFields[name] {
						fieldVar, ok := mappedFieldTypes[field]
						if !ok {
							return fmt.Errorf("expected struct %q to contain field %q", name, field)
						}

						accessorStructName := mirrorStructName
						if mappedFieldIndexes[field] == 0 && compactFieldNames[name][field] {
							accessorStructName = mirrorStructName + "_" + field
							shimBuilder.WriteString("type ")
							shimBuilder.WriteString(accessorStructName)
							shimBuilder.WriteString(" struct {\n  ")
							shimBuilder.WriteString(field)
							shimBuilder.WriteByte(' ')
							shimBuilder.WriteString(types.TypeString(fieldVar.Type(), qualifierOnlyPackageName))
							shimBuilder.WriteString("\n}\n")
						}

						shimBuilder.WriteString("func ")
						shimBuilder.WriteString(name)
						shimBuilder.WriteByte('_')
						shimBuilder.WriteString(field)
						shimBuilder.WriteString("(v *")
						shimBuilder.WriteString(pkg.Name)
						shimBuilder.WriteByte('.')
						shimBuilder.WriteString(name)
						shimBuilder.WriteString(") ")
						shimBuilder.WriteString(types.TypeString(fieldVar.Type(), qualifierOnlyPackageName))
						shimBuilder.WriteString(" {\n")
						shimBuilder.WriteString("  return ((*")
						shimBuilder.WriteString(accessorStructName)
						shimBuilder.WriteString(")(unsafe.Pointer(v))).")
						shimBuilder.WriteString(field)
						shimBuilder.WriteString("\n")
						shimBuilder.WriteString("}\n")
					}
				}
			case *types.Const:
				printReexport("const")
			case *types.Var:
				printReexport("var")
			case *types.Func:
				if !slices.Contains(extraShim.IgnoreFunctions, name) {
					funcType := object.(*types.Func)
					emitLinkedFunction(funcType)
				}
			}
		}

		exit := false
		for fnName, found := range matchedExtraFunctions {
			if found {
				continue
			}
			fmt.Printf("ERROR: couldn't find %v function\n", fnName)
			exit = true
		}
		for name, methods := range matchedExtraMethods {
			for methodName, found := range methods {
				if found {
					continue
				}
				fmt.Printf("ERROR: couldn't find %v.%v method\n", name, methodName)
				exit = true
			}
		}
		if exit {
			return fmt.Errorf("extra shim declarations were not found in %s", packagePath)
		}

		// https://pkg.go.dev/cmd/go#hdr-Generate_Go_files_by_processing_source
		shimHeaderBuilder.WriteString("\n// Code generated by _tools/gen_shims. DO NOT EDIT.\n\n")
		shimHeaderBuilder.WriteString("package ")
		shimHeaderBuilder.WriteString(pkg.Name)
		shimHeaderBuilder.WriteString("\n\n")
		importsList := slices.Collect(maps.Keys(importedPackages))
		slices.Sort(importsList)
		for _, imported := range importsList {
			shimHeaderBuilder.WriteString("import ")
			if !importedPackages[imported] {
				shimHeaderBuilder.WriteString("_ ")
			}
			shimHeaderBuilder.WriteString("\"")
			shimHeaderBuilder.WriteString(imported)
			shimHeaderBuilder.WriteString("\"\n")
		}
		shimHeaderBuilder.WriteString("\n")

		generatedSource := append([]byte(shimHeaderBuilder.String()), shimBuilder.String()...)
		shimGoPath := filepath.Join(shimDirPath, "shim.go")
		if _, err := parser.ParseFile(token.NewFileSet(), shimGoPath, generatedSource, parser.AllErrors); err != nil {
			return fmt.Errorf("validate generated shim %s: %w", packagePath, err)
		}
		if err := writeFile(shimGoPath, generatedSource, 0o644); err != nil {
			return fmt.Errorf("write generated shim %s: %w", packagePath, err)
		}

		shimHeaderBuilder.Reset()
		shimBuilder.Reset()
	}

	for _, modulePath := range modulePaths() {
		goMod := fmt.Sprintf("module github.com/microsoft/typescript-go/shim/%s\n\ngo 1.26\n\nrequire github.com/microsoft/typescript-go v0.0.0\n", filepath.ToSlash(modulePath))
		if err := writeFile(filepath.Join(shimPath, filepath.FromSlash(modulePath), "go.mod"), []byte(goMod), 0o644); err != nil {
			return fmt.Errorf("write go.mod for %s: %w", modulePath, err)
		}
	}
	if err := writeFile(filepath.Join(shimPath, generatedMarker), []byte("Generated by _tools/gen_shims. DO NOT EDIT.\n"), 0o644); err != nil {
		return err
	}
	if err := writeFile(goWorkPath, updatedGoWork, 0o644); err != nil {
		return fmt.Errorf("update go.work: %w", err)
	}
	return nil
}
