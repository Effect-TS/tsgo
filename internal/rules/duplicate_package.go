package rules

import (
	"fmt"
	"slices"
	"strings"
	"sync"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// duplicatePackageCache caches duplicate-package diagnostics for the current
// program check cycle so the package scan is not repeated for every source file.
// Only the most recent program's result is kept, preventing memory leaks when
// many programs are created (e.g., in tests).
var duplicatePackageCacheMu sync.Mutex
var duplicatePackageCacheProg checker.Program
var duplicatePackageCacheResult []duplicatePackageDiag

// duplicatePackageDiag holds pre-computed diagnostic info for a single duplicated package name.
type duplicatePackageDiag struct {
	packageName string
	details     string // e.g. "1.0.0 @ /path/a, 2.0.0 @ /path/b"
}

// DuplicatePackage warns when multiple versions of the same Effect-related package
// are loaded into the program.
var DuplicatePackage = rule.Rule{
	Name:            "duplicatePackage",
	Description:     "Warns when multiple versions of an Effect-related package are detected in the program",
	DefaultSeverity: etscore.SeverityWarning,
	Codes:           []int32{tsdiag.Multiple_versions_of_package_0_detected_Colon_1_Consider_cleaning_up_your_lockfile_or_add_0_to_allowedDuplicatedPackages_to_suppress_this_warning_effect_duplicatePackage.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		prog := ctx.Checker.Program()
		entries := getDuplicatePackageDiags(ctx.Checker, prog)
		if len(entries) == 0 {
			return nil
		}

		// Attach one diagnostic per duplicated package to the first statement (or the source file node).
		var target *ast.Node
		stmts := ctx.SourceFile.Statements.Nodes
		if len(stmts) > 0 {
			target = stmts[0]
		} else {
			target = ctx.SourceFile.AsNode()
		}
		loc := ctx.GetErrorRange(target)

		diags := make([]*ast.Diagnostic, len(entries))
		for i, e := range entries {
			diags[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				loc,
				tsdiag.Multiple_versions_of_package_0_detected_Colon_1_Consider_cleaning_up_your_lockfile_or_add_0_to_allowedDuplicatedPackages_to_suppress_this_warning_effect_duplicatePackage,
				nil,
				e.packageName,
				e.details,
			)
		}
		return diags
	},
}

// ClearDuplicatePackageCache removes the cached duplicate-package diagnostics,
// allowing the associated program to be garbage collected. Call this after
// diagnostics collection is complete (e.g., from ReleaseProgram).
func ClearDuplicatePackageCache() {
	duplicatePackageCacheMu.Lock()
	defer duplicatePackageCacheMu.Unlock()
	duplicatePackageCacheProg = nil
	duplicatePackageCacheResult = nil
}

// getDuplicatePackageDiags returns cached duplicate-package diagnostics for the given program.
// The cache holds at most one entry (the current program), so old programs are released for GC.
func getDuplicatePackageDiags(c *checker.Checker, prog checker.Program) []duplicatePackageDiag {
	duplicatePackageCacheMu.Lock()
	defer duplicatePackageCacheMu.Unlock()

	if duplicatePackageCacheProg == prog {
		return duplicatePackageCacheResult
	}

	result := computeDuplicatePackageDiags(c, prog)
	duplicatePackageCacheProg = prog
	duplicatePackageCacheResult = result
	return result
}

// computeDuplicatePackageDiags scans all packages and finds names with multiple distinct versions.
func computeDuplicatePackageDiags(c *checker.Checker, prog checker.Program) []duplicatePackageDiag {
	packages := typeparser.DiscoverPackages(c)

	// Filter to Effect-related packages.
	type versionEntry struct {
		version string
		dir     string
	}
	byName := map[string][]versionEntry{}
	for _, pkg := range packages {
		if pkg.Name != "effect" && !pkg.DependsOnEffect {
			continue
		}
		ver := ""
		if pkg.Version != nil {
			ver = *pkg.Version
		}
		byName[pkg.Name] = append(byName[pkg.Name], versionEntry{version: ver, dir: pkg.PackageDirectory})
	}

	// Read allowed list from config.
	effectConfig := getEffectConfig(prog)
	var allowed []string
	if effectConfig != nil {
		allowed = effectConfig.GetAllowedDuplicatedPackages()
	}

	var diags []duplicatePackageDiag

	// Sort names for deterministic ordering.
	names := make([]string, 0, len(byName))
	for name := range byName {
		names = append(names, name)
	}
	slices.Sort(names)

	for _, name := range names {
		entries := byName[name]
		if len(entries) <= 1 {
			continue
		}
		if slices.Contains(allowed, name) {
			continue
		}

		// Build details string: "1.0.0 @ /path/a, 2.0.0 @ /path/b"
		parts := make([]string, len(entries))
		for i, e := range entries {
			if e.version != "" {
				parts[i] = fmt.Sprintf("%s @ %s", e.version, e.dir)
			} else {
				parts[i] = fmt.Sprintf("(unknown) @ %s", e.dir)
			}
		}
		diags = append(diags, duplicatePackageDiag{
			packageName: name,
			details:     strings.Join(parts, ", "),
		})
	}

	return diags
}
