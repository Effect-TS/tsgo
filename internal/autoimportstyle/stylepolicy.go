package autoimportstyle

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/ls/autoimport"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/modulespecifiers"
)

// StylePreferences holds the auto-import style configuration parsed from Effect plugin options.
type StylePreferences = modulespecifiers.EffectAutoImportStylePreferences

// PreferencesFromPluginOptions converts Effect plugin options into StylePreferences
// for the auto-import style policy. Returns zero-value preferences if opts is nil.
func PreferencesFromPluginOptions(opts *etscore.EffectPluginOptions) StylePreferences {
	if opts == nil {
		return StylePreferences{}
	}
	return StylePreferences{
		NamespaceImportPackages: opts.GetNamespaceImportPackages(),
		BarrelImportPackages:    opts.GetBarrelImportPackages(),
		ImportAliases:           opts.GetImportAliases(),
		FollowTopLevelReexports: opts.GetTopLevelNamedReexports() == etscore.TopLevelNamedReexportsFollow,
	}
}

// stylePolicy applies auto-import style rewrites based on configured preferences.
type stylePolicy struct {
	namespacePackages map[string]bool
	barrelPackages    map[string]bool
	aliases           map[string]string
	followReexports   bool
}

// NewFixTransformer creates a FixTransformer from the given style preferences.
// Returns nil if the preferences are empty (no packages configured).
func NewFixTransformer(prefs StylePreferences) autoimport.FixTransformer {
	sp := newStylePolicy(prefs)
	if sp.isEmpty() {
		return nil
	}
	return func(export *autoimport.Export, fixes []*autoimport.Fix) []*autoimport.Fix {
		rewritten := make([]*autoimport.Fix, 0, len(fixes))
		for _, fix := range fixes {
			adjusted := sp.Apply(export, fix)
			if adjusted != nil {
				rewritten = append(rewritten, adjusted)
			}
		}
		if len(rewritten) == 0 {
			return nil
		}
		return rewritten
	}
}

// newStylePolicy creates a stylePolicy from the given preferences.
// Package names are lowercased for case-insensitive matching.
func newStylePolicy(prefs StylePreferences) *stylePolicy {
	sp := &stylePolicy{
		namespacePackages: make(map[string]bool, len(prefs.NamespaceImportPackages)),
		barrelPackages:    make(map[string]bool, len(prefs.BarrelImportPackages)),
		aliases:           make(map[string]string, len(prefs.ImportAliases)),
		followReexports:   prefs.FollowTopLevelReexports,
	}
	for _, pkg := range prefs.NamespaceImportPackages {
		sp.namespacePackages[strings.ToLower(pkg)] = true
	}
	for _, pkg := range prefs.BarrelImportPackages {
		sp.barrelPackages[strings.ToLower(pkg)] = true
	}
	for pkg, alias := range prefs.ImportAliases {
		sp.aliases[strings.ToLower(pkg)] = alias
	}
	return sp
}

// isEmpty returns true if no style preferences are configured.
func (sp *stylePolicy) isEmpty() bool {
	return sp == nil || (len(sp.namespacePackages) == 0 && len(sp.barrelPackages) == 0)
}

// Apply transforms a fix based on the export's package and the configured style.
// Returns nil if the fix should be suppressed (e.g., ignored top-level reexport).
// Returns the fix unchanged if no style policy applies.
func (sp *stylePolicy) Apply(export *autoimport.Export, fix *autoimport.Fix) *autoimport.Fix {
	if sp.isEmpty() || fix == nil || export == nil {
		return fix
	}

	// Only rewrite AddNew fixes
	if fix.Kind != lsproto.AutoImportFixKindAddNew {
		return fix
	}

	pkgName := strings.ToLower(export.PackageName)
	if pkgName == "" {
		return fix
	}

	// Check namespace-import packages
	if sp.namespacePackages[pkgName] {
		return sp.applyNamespaceRewrite(export, fix)
	}

	// Check barrel-import packages
	if sp.barrelPackages[pkgName] {
		return sp.applyBarrelRewrite(export, fix)
	}

	return fix
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

	// Check if this is a top-level named reexport
	isReexport := export.Target.ModuleID != "" && export.Target.ModuleID != export.ModuleID
	if isReexport && !sp.followReexports {
		// When topLevelNamedReexports is "ignore", skip rewriting top-level reexports
		return fix
	}
	if isReexport && sp.followReexports {
		// When topLevelNamedReexports is "follow", suppress reexport fixes
		// so the direct submodule namespace import wins instead
		return nil
	}

	namespaceName := inferNamespaceName(fix.ModuleSpecifier)
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

// applyBarrelRewrite rewrites a fix to a named import from the barrel package.
func (sp *stylePolicy) applyBarrelRewrite(export *autoimport.Export, fix *autoimport.Fix) *autoimport.Fix {
	// Barrel rewrites require usage qualification (e.g. `request` -> `HttpClient.request`).
	// If no usage site is available, keep the original named-import fix.
	if fix.UsagePosition == nil {
		return fix
	}

	barrelSpecifier := export.PackageName
	if barrelSpecifier == "" {
		return fix
	}

	// Infer the namespace name from the module specifier (e.g., "effect/Effect" -> "Effect")
	namespaceName := inferNamespaceName(fix.ModuleSpecifier)
	if namespaceName == "" {
		return fix
	}

	// Apply alias if configured
	pkgName := strings.ToLower(export.PackageName)
	alias := sp.resolveAlias(pkgName, namespaceName)

	result := &autoimport.Fix{
		AutoImportFix: &lsproto.AutoImportFix{
			Kind:            lsproto.AutoImportFixKindAddNew,
			ImportKind:      lsproto.ImportKindNamed,
			ModuleSpecifier: barrelSpecifier,
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

// resolveAlias returns the configured alias for the given namespace name under
// the given package, or the namespace name itself if no alias is configured.
func (sp *stylePolicy) resolveAlias(pkgNameLower string, namespaceName string) string {
	if alias, ok := sp.aliases[pkgNameLower]; ok {
		return alias
	}
	return namespaceName
}

// packageNameFromSpecifier extracts the npm package name from a bare module specifier.
// For scoped packages like "@scope/pkg/sub", returns "@scope/pkg".
// For non-scoped packages like "pkg/sub", returns "pkg".
// For relative paths, returns "".
func packageNameFromSpecifier(specifier string) string {
	if specifier == "" || specifier[0] == '.' || specifier[0] == '/' {
		return ""
	}
	if specifier[0] == '@' {
		// Scoped package: @scope/name or @scope/name/sub
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
	// Non-scoped: name or name/sub
	name, _, found := strings.Cut(specifier, "/")
	if !found {
		return specifier
	}
	return name
}

// inferNamespaceName derives a namespace name from the last segment of a module specifier path.
// For example, "effect/Effect" → "Effect", "effect" → "effect", "@scope/pkg/Foo" → "Foo".
func inferNamespaceName(moduleSpecifier string) string {
	if moduleSpecifier == "" {
		return ""
	}
	lastSlash := strings.LastIndex(moduleSpecifier, "/")
	if lastSlash < 0 {
		return moduleSpecifier
	}
	segment := moduleSpecifier[lastSlash+1:]
	if segment == "" {
		return ""
	}
	return segment
}
