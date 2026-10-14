// Package importstyle classifies module specifiers against the configured
// namespaceImportPackages and barrelImportPackages options.
package importstyle

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
)

type Style uint8

const (
	StyleNone Style = iota
	StyleNamespace
	StyleBarrel
)

// Policy holds the configured package lists. It references the option slices
// directly, so building one per file costs nothing.
type Policy struct {
	namespacePackages []string
	barrelPackages    []string
}

func NewPolicy(options *etscore.ResolvedEffectPluginOptions) Policy {
	if options == nil {
		return Policy{}
	}
	return Policy{
		namespacePackages: options.NamespaceImportPackages,
		barrelPackages:    options.BarrelImportPackages,
	}
}

func (p Policy) IsEmpty() bool {
	return len(p.namespacePackages) == 0 && len(p.barrelPackages) == 0
}

// PackageStyle returns the style configured for a package name. Matching is
// case-insensitive, and a package listed in both options uses namespace imports.
func (p Policy) PackageStyle(packageName string) Style {
	if packageName == "" {
		return StyleNone
	}
	if containsFold(p.namespacePackages, packageName) {
		return StyleNamespace
	}
	if containsFold(p.barrelPackages, packageName) {
		return StyleBarrel
	}
	return StyleNone
}

func (p Policy) Style(specifier string) Style {
	return p.PackageStyle(PackageName(specifier))
}

func containsFold(packages []string, packageName string) bool {
	for _, pkg := range packages {
		if strings.EqualFold(pkg, packageName) {
			return true
		}
	}
	return false
}

// PackageName extracts the npm package name from a bare module specifier.
// For scoped packages like "@scope/pkg/sub", returns "@scope/pkg".
// For non-scoped packages like "pkg/sub", returns "pkg".
// For relative and absolute paths, returns "".
func PackageName(specifier string) string {
	if specifier == "" || specifier[0] == '.' || specifier[0] == '/' {
		return ""
	}
	if specifier[0] == '@' {
		slashIdx := strings.Index(specifier, "/")
		if slashIdx < 0 {
			return specifier
		}
		secondSlash := strings.Index(specifier[slashIdx+1:], "/")
		if secondSlash < 0 {
			return specifier
		}
		return specifier[:slashIdx+1+secondSlash]
	}
	name, _, _ := strings.Cut(specifier, "/")
	return name
}

// NamespaceName derives a namespace name from the last segment of a module specifier.
// For example, "effect/Effect" -> "Effect", "effect" -> "effect", "@scope/pkg/Foo" -> "Foo".
func NamespaceName(specifier string) string {
	return specifier[strings.LastIndex(specifier, "/")+1:]
}

var declarationExtensions = []string{".d.ts", ".d.mts", ".d.cts", ".ts", ".mts", ".cts", ".js", ".mjs", ".cjs"}

// ReexportSpecifier maps a sibling re-export inside a barrel ("./Effect.ts" in
// the index of "effect") to its public specifier ("effect/Effect"). It returns
// "" when the target is not a direct sibling, since only siblings map onto the
// package's subpath exports one to one.
func ReexportSpecifier(barrel string, relative string) string {
	name, ok := strings.CutPrefix(relative, "./")
	if !ok || barrel == "" {
		return ""
	}
	for _, extension := range declarationExtensions {
		if trimmed, found := strings.CutSuffix(name, extension); found {
			name = trimmed
			break
		}
	}
	if name == "" || name == "index" || strings.ContainsAny(name, "/.") {
		return ""
	}
	return barrel + "/" + name
}
