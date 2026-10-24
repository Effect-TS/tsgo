package autoimportstyle

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/importstyle"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/ls/autoimport"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/modulespecifiers"
)

// stylePolicy applies auto-import style rewrites based on configured preferences.
type stylePolicy struct {
	styles          importstyle.Policy
	aliases         map[string]string
	followReexports bool
	barrels         *importstyle.BarrelReader
	resolveTarget   func(export *autoimport.Export) (string, modulespecifiers.ResultKind)
}

// NewFixTransformer creates a FixTransformer from the given resolved Effect options.
// Returns nil if the preferences are empty (no packages configured).
func NewFixTransformer(
	resolved *etscore.ResolvedEffectPluginOptions,
	program importstyle.ProgramFiles,
	resolveTarget func(export *autoimport.Export) (string, modulespecifiers.ResultKind),
) autoimport.FixTransformer {
	sp := newStylePolicy(resolved)
	if program != nil {
		sp.barrels = importstyle.NewProgramBarrelReader(program)
	}
	sp.resolveTarget = resolveTarget
	if sp.isEmpty() {
		return nil
	}
	return func(export *autoimport.Export, fixes []*autoimport.Fix) []*autoimport.Fix {
		rewritten := make([]*autoimport.Fix, 0, len(fixes))
		hasUseNamespace := make(map[string]bool)
		for _, fix := range fixes {
			adjusted := sp.Apply(export, fix)
			if adjusted != nil {
				if adjusted.Kind == lsproto.AutoImportFixKindUseNamespace {
					hasUseNamespace[namespaceFixKey(adjusted)] = true
				}
				rewritten = append(rewritten, adjusted)
			}
		}
		if len(hasUseNamespace) != 0 {
			filtered := rewritten[:0]
			for _, fix := range rewritten {
				if fix.Kind == lsproto.AutoImportFixKindAddNew && fix.ImportKind == lsproto.ImportKindNamespace && hasUseNamespace[namespaceFixKey(fix)] {
					continue
				}
				filtered = append(filtered, fix)
			}
			rewritten = filtered
		}
		if len(rewritten) == 0 {
			return nil
		}
		return rewritten
	}
}

func namespaceFixKey(fix *autoimport.Fix) string {
	if fix == nil {
		return ""
	}
	return fix.ModuleSpecifier + "\x00" + fix.NamespacePrefix
}

// newStylePolicy creates a stylePolicy from the given resolved options.
// Alias keys are lowercased for case-insensitive matching.
func newStylePolicy(resolved *etscore.ResolvedEffectPluginOptions) *stylePolicy {
	if resolved == nil {
		return &stylePolicy{}
	}
	sp := &stylePolicy{
		styles:          importstyle.NewPolicy(resolved),
		aliases:         make(map[string]string, len(resolved.ImportAliases)),
		followReexports: resolved.TopLevelNamedReexports == etscore.TopLevelNamedReexportsFollow,
	}
	for pkg, alias := range resolved.ImportAliases {
		sp.aliases[strings.ToLower(pkg)] = alias
	}
	return sp
}

// isEmpty returns true if no style preferences are configured.
func (sp *stylePolicy) isEmpty() bool {
	return sp == nil || sp.styles.IsEmpty()
}

// Apply transforms a fix based on the export's package and the configured style.
// Returns nil if the fix should be suppressed (e.g., ignored top-level reexport).
// Returns the fix unchanged if no style policy applies.
func (sp *stylePolicy) Apply(export *autoimport.Export, fix *autoimport.Fix) *autoimport.Fix {
	if sp.isEmpty() || fix == nil || export == nil {
		return fix
	}

	// Only rewrite new imports or additions to existing imports.
	if fix.Kind != lsproto.AutoImportFixKindAddNew && fix.Kind != lsproto.AutoImportFixKindAddToExisting {
		return fix
	}

	style := sp.styles.PackageStyle(export.PackageName)
	if style == importstyle.StyleNone {
		return fix
	}
	if strings.EqualFold(export.PackageName, "effect") && isEffectInternal(fix.ModuleSpecifier) {
		return nil
	}
	if style == importstyle.StyleNamespace {
		return sp.applyNamespaceRewrite(export, fix)
	}
	return sp.applyBarrelRewrite(export, fix)
}

// applyNamespaceRewrite rewrites a named-import fix to a namespace-import fix.
func (sp *stylePolicy) applyNamespaceRewrite(export *autoimport.Export, fix *autoimport.Fix) *autoimport.Fix {
	// Only rewrite named imports
	if fix.ImportKind != lsproto.ImportKindNamed {
		return fix
	}
	// Rewrites require usage qualification (e.g. `succeed` -> `Effect.succeed`).
	// If no usage site is available, keep the original named-import fix.
	if fix.UsagePosition == nil {
		return fix
	}
	if isNamespaceReexport(export) {
		if sp.resolveTarget == nil {
			return fix
		}
		moduleSpecifier, moduleSpecifierKind := sp.resolveTarget(export)
		if moduleSpecifier == "" {
			return fix
		}
		return &autoimport.Fix{
			AutoImportFix: &lsproto.AutoImportFix{
				Kind:            lsproto.AutoImportFixKindAddNew,
				ImportKind:      lsproto.ImportKindNamespace,
				ModuleSpecifier: moduleSpecifier,
				Name:            export.Name(),
				UseRequire:      fix.UseRequire,
				AddAsTypeOnly:   fix.AddAsTypeOnly,
				UsagePosition:   fix.UsagePosition,
			},
			ModuleSpecifierKind: moduleSpecifierKind,
			ModuleFileName:      autoimport.ModuleIDFileName(export.Target.ModuleID),
		}
	}

	// Check if this is a top-level named reexport
	isReexport := autoimport.ModuleIDString(export.Target.ModuleID) != "" && export.Target.ModuleID != export.ModuleID
	if isReexport && !sp.followReexports {
		// When topLevelNamedReexports is "ignore", skip rewriting top-level reexports
		return fix
	}
	if isReexport && sp.followReexports {
		// When topLevelNamedReexports is "follow", suppress reexport fixes
		// so the direct submodule namespace import wins instead
		return nil
	}

	namespaceName := importstyle.NamespaceName(fix.ModuleSpecifier)
	if namespaceName == "" {
		return fix
	}

	// Apply alias if configured
	pkgName := strings.ToLower(export.PackageName)
	alias := sp.resolveAlias(pkgName, namespaceName)

	result := &autoimport.Fix{
		AutoImportFix: &lsproto.AutoImportFix{
			Kind:            lsproto.AutoImportFixKindAddNew,
			ImportKind:      lsproto.ImportKindNamespace,
			ModuleSpecifier: fix.ModuleSpecifier,
			Name:            fix.Name,
			UseRequire:      fix.UseRequire,
			AddAsTypeOnly:   fix.AddAsTypeOnly,
			UsagePosition:   fix.UsagePosition,
			NamespacePrefix: alias,
		},
		ModuleSpecifierKind: fix.ModuleSpecifierKind,
		IsReExport:          fix.IsReExport,
		ModuleFileName:      fix.ModuleFileName,
	}
	return result
}

func isNamespaceReexport(export *autoimport.Export) bool {
	return export != nil &&
		export.Syntax == autoimport.ExportSyntaxModifier &&
		export.Flags&ast.SymbolFlagsModule != 0 &&
		autoimport.ModuleIDString(export.Target.ModuleID) != "" &&
		export.Target.ModuleID != export.ModuleID
}

// applyBarrelRewrite rewrites a fix to a named import from the barrel that
// forwards the fix's module, keeping the fix unchanged when no barrel is found.
func (sp *stylePolicy) applyBarrelRewrite(export *autoimport.Export, fix *autoimport.Fix) *autoimport.Fix {
	// Barrel rewrites require usage qualification (e.g. `request` -> `HttpClient.request`).
	// If no usage site is available, keep the original named-import fix.
	if fix.UsagePosition == nil {
		return fix
	}
	barrel, ok := sp.barrels.Find(fix.ModuleSpecifier, fix.ModuleFileName)
	if !ok {
		return fix
	}

	pkgName := strings.ToLower(export.PackageName)
	alias := sp.resolveAlias(pkgName, barrel.ExportName)

	result := &autoimport.Fix{
		AutoImportFix: &lsproto.AutoImportFix{
			Kind:            lsproto.AutoImportFixKindAddNew,
			ImportKind:      lsproto.ImportKindNamed,
			ModuleSpecifier: barrel.Specifier,
			Name:            fix.Name,
			UseRequire:      fix.UseRequire,
			AddAsTypeOnly:   fix.AddAsTypeOnly,
			UsagePosition:   fix.UsagePosition,
			NamespacePrefix: alias,
		},
		ModuleSpecifierKind: fix.ModuleSpecifierKind,
		IsReExport:          fix.IsReExport,
		ModuleFileName:      fix.ModuleFileName,
	}
	return result
}

// isEffectInternal reports whether a specifier points into an internal
// directory of the effect package, which auto-import never suggests.
func isEffectInternal(specifier string) bool {
	return strings.HasPrefix(specifier, "effect/internal/") || strings.Contains(specifier, "/internal/")
}

// resolveAlias returns the configured alias for the given namespace name under
// the given package, or the namespace name itself if no alias is configured.
func (sp *stylePolicy) resolveAlias(pkgNameLower string, namespaceName string) string {
	if alias, ok := sp.aliases[pkgNameLower]; ok {
		return alias
	}
	return namespaceName
}
