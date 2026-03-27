package rules

import (
	"fmt"
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// duplicatePackageDiag holds pre-computed diagnostic info for a single duplicated package name.
type duplicatePackageDiag struct {
	packageName string
	details     string // e.g. "1.0.0 @ /path/a, 2.0.0 @ /path/b"
}

// DuplicatePackage warns when multiple versions of the same Effect-related package
// are loaded into the program.
var DuplicatePackage = rule.Rule{
	Name:            "duplicatePackage",
	Group:           "correctness",
	Description:     "Warns when multiple versions of an Effect-related package are detected in the program",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Multiple_versions_of_package_0_detected_Colon_1_Consider_cleaning_up_your_lockfile_or_add_0_to_allowedDuplicatedPackages_to_suppress_this_warning_effect_duplicatePackage.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		entries := computeDuplicatePackageDiags(ctx.Checker, ctx.Checker.Program())
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
				parts[i] = "(unknown) @ " + e.dir
			}
		}
		diags = append(diags, duplicatePackageDiag{
			packageName: name,
			details:     strings.Join(parts, ", "),
		})
	}

	return diags
}
