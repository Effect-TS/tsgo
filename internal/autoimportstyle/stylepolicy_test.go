package autoimportstyle

import (
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/importstyle"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/ls/autoimport"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/modulespecifiers"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

func TestNewStylePolicy(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect", "Effect"},
		BarrelImportPackages:    []string{"@effect/platform"},
		ImportAliases:           map[string]string{"effect": "Fx"},
	})

	if sp.styles.PackageStyle("effect") != importstyle.StyleNamespace {
		t.Error("expected 'effect' in namespace packages")
	}
	if sp.styles.PackageStyle("EFFECT") != importstyle.StyleNamespace {
		t.Error("expected case-insensitive match for 'EFFECT'")
	}
	if sp.styles.PackageStyle("@effect/platform") != importstyle.StyleBarrel {
		t.Error("expected '@effect/platform' in barrel packages")
	}
	if sp.aliases["effect"] != "Fx" {
		t.Errorf("expected alias 'Fx' for 'effect', got %q", sp.aliases["effect"])
	}
}

func TestStylePolicyIsEmpty(t *testing.T) {
	t.Parallel()
	var nilPolicy *stylePolicy
	if !nilPolicy.isEmpty() {
		t.Error("nil policy should be empty")
	}

	empty := newStylePolicy(&etscore.ResolvedEffectPluginOptions{})
	if !empty.isEmpty() {
		t.Error("empty preferences should produce empty policy")
	}

	nonEmpty := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})
	if nonEmpty.isEmpty() {
		t.Error("non-empty preferences should not be empty")
	}
}

func makeExport(packageName string, moduleID string, targetModuleID string) *autoimport.Export {
	return &autoimport.Export{
		ModuleID:    autoimport.NewAmbientModuleID(moduleID),
		ExportName:  "succeed",
		PackageName: packageName,
		Target: autoimport.ExportID{
			ModuleID: autoimport.NewAmbientModuleID(targetModuleID),
		},
	}
}

func makeAddNewFix(importKind lsproto.ImportKind, moduleSpecifier string, name string) *autoimport.Fix {
	usagePosition := &lsproto.Position{Line: 1, Character: 1}
	return &autoimport.Fix{
		AutoImportFix: &lsproto.AutoImportFix{
			Kind:            lsproto.AutoImportFixKindAddNew,
			ImportKind:      importKind,
			ModuleSpecifier: moduleSpecifier,
			Name:            name,
			UsagePosition:   usagePosition,
		},
	}
}

func TestApplyNamespaceRewrite(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	} else {
		if result.ImportKind != lsproto.ImportKindNamespace {
			t.Errorf("expected ImportKindNamespace, got %v", result.ImportKind)
		}
		if result.Name != "succeed" {
			t.Errorf("expected name 'succeed' (original export name), got %q", result.Name)
		}
		if result.NamespacePrefix != "Effect" {
			t.Errorf("expected namespace prefix 'Effect', got %q", result.NamespacePrefix)
		}
		if result.ModuleSpecifier != "effect/Effect" {
			t.Errorf("expected module specifier 'effect/Effect', got %q", result.ModuleSpecifier)
		}
	}
}

func TestApplySuppressesEffectInternalImport(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("effect", "effect/internal/schema/parser", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/internal/schema/parser", "succeed")

	if result := sp.Apply(export, fix); result != nil {
		t.Errorf("expected blocked Effect internal import to be suppressed, got %#v", result)
	}
}

func TestApplyNamespaceRewriteFromAddToExisting(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("effect", "effect/testing/TestClock", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/testing/TestClock", "testClockWith")
	fix.Kind = lsproto.AutoImportFixKindAddToExisting

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
		return
	}
	if result.Kind != lsproto.AutoImportFixKindAddNew {
		t.Errorf("expected rewrite to AddNew, got %v", result.Kind)
	}
	if result.ImportKind != lsproto.ImportKindNamespace {
		t.Errorf("expected ImportKindNamespace, got %v", result.ImportKind)
	}
	if result.ModuleSpecifier != "effect/testing/TestClock" {
		t.Errorf("expected module specifier 'effect/testing/TestClock', got %q", result.ModuleSpecifier)
	}
	if result.NamespacePrefix != "TestClock" {
		t.Errorf("expected namespace prefix 'TestClock', got %q", result.NamespacePrefix)
	}
}

func TestApplyNamespaceRewriteWithAlias(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
		ImportAliases:           map[string]string{"effect": "Fx"},
	})

	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	} else {
		if result.ImportKind != lsproto.ImportKindNamespace {
			t.Errorf("expected ImportKindNamespace, got %v", result.ImportKind)
		}
		if result.Name != "succeed" {
			t.Errorf("expected name 'succeed' (original export name), got %q", result.Name)
		}
		if result.NamespacePrefix != "Fx" {
			t.Errorf("expected namespace prefix 'Fx', got %q", result.NamespacePrefix)
		}
	}
}

func TestApplyNamespaceReexport(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})
	sp.resolveTarget = func(export *autoimport.Export) (string, modulespecifiers.ResultKind) {
		if autoimport.ModuleIDString(export.Target.ModuleID) != "effect/Effect" {
			t.Fatalf("unexpected target module: %q", export.Target.ModuleID)
		}
		return "effect/Effect", modulespecifiers.ResultKindNodeModules
	}

	export := makeExport("effect", "effect", "effect/Effect")
	export.ExportName = "Effect"
	export.Syntax = autoimport.ExportSyntaxModifier
	export.Flags = ast.SymbolFlagsValueModule
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect", "Effect")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.ImportKind != lsproto.ImportKindNamespace {
		t.Errorf("expected ImportKindNamespace, got %v", result.ImportKind)
	}
	if result.ModuleSpecifier != "effect/Effect" {
		t.Errorf("expected module specifier 'effect/Effect', got %q", result.ModuleSpecifier)
	}
	if result.Name != "Effect" {
		t.Errorf("expected namespace name 'Effect', got %q", result.Name)
	}
	if result.NamespacePrefix != "" {
		t.Errorf("expected no usage qualifier, got %q", result.NamespacePrefix)
	}
	if result.ModuleSpecifierKind != modulespecifiers.ResultKindNodeModules {
		t.Errorf("expected node_modules specifier kind, got %v", result.ModuleSpecifierKind)
	}
	if result.IsReExport {
		t.Error("expected rewritten direct import not to be marked as a reexport")
	}
}

func newBarrelPolicy(options *etscore.ResolvedEffectPluginOptions, disk map[string]string) *stylePolicy {
	sp := newStylePolicy(options)
	sp.barrels = importstyle.NewBarrelReader(
		func(tspath.RootedFilePath) *ast.SourceFile { return nil },
		func(fileName tspath.RootedFilePath) (string, bool) {
			text, ok := disk[string(fileName)]
			return text, ok
		},
	)
	return sp
}

func makeBarrelFix(moduleSpecifier string, moduleFileName string, name string) *autoimport.Fix {
	fix := makeAddNewFix(lsproto.ImportKindNamed, moduleSpecifier, name)
	fix.ModuleFileName = tspath.RootedFilePath(moduleFileName)
	return fix
}

func TestApplyBarrelRewrite(t *testing.T) {
	t.Parallel()
	sp := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"@effect/platform"},
	}, map[string]string{"/node_modules/@effect/platform/dist/dts/index.d.ts": `export * as HttpClient from "./HttpClient.js"`})

	export := makeExport("@effect/platform", "@effect/platform/HttpClient", "")
	fix := makeBarrelFix("@effect/platform/HttpClient", "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts", "request")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.ImportKind != lsproto.ImportKindNamed {
		t.Errorf("expected ImportKindNamed, got %v", result.ImportKind)
	}
	if result.ModuleSpecifier != "@effect/platform" {
		t.Errorf("expected module specifier '@effect/platform', got %q", result.ModuleSpecifier)
	}
	if result.Name != "request" {
		t.Errorf("expected name 'request' (original export name), got %q", result.Name)
	}
	if result.NamespacePrefix != "HttpClient" {
		t.Errorf("expected namespace prefix 'HttpClient', got %q", result.NamespacePrefix)
	}
}

func TestApplyBarrelRewriteUsesNearestForwardingBarrel(t *testing.T) {
	t.Parallel()
	sp := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"acme"},
	}, map[string]string{
		"/node_modules/acme/dist/index.d.ts":       `export * as Http from "./HttpClient.ts"`,
		"/node_modules/acme/dist/forms/index.d.ts": `export * as TextInput from "./TextInput.ts"`,
	})

	tests := []struct {
		moduleSpecifier string
		moduleFileName  string
		barrel          string
		prefix          string
	}{
		{"acme/HttpClient", "/node_modules/acme/dist/HttpClient.d.ts", "acme", "Http"},
		{"acme/forms/TextInput", "/node_modules/acme/dist/forms/TextInput.d.ts", "acme/forms", "TextInput"},
	}
	for _, tt := range tests {
		result := sp.Apply(makeExport("acme", tt.moduleSpecifier, ""), makeBarrelFix(tt.moduleSpecifier, tt.moduleFileName, "make"))
		if result == nil {
			t.Fatalf("%s: expected non-nil result", tt.moduleSpecifier)
		}
		if result.ModuleSpecifier != tt.barrel {
			t.Errorf("%s: expected module specifier %q, got %q", tt.moduleSpecifier, tt.barrel, result.ModuleSpecifier)
		}
		if result.NamespacePrefix != tt.prefix {
			t.Errorf("%s: expected namespace prefix %q, got %q", tt.moduleSpecifier, tt.prefix, result.NamespacePrefix)
		}
	}
}

func TestApplyBarrelRewriteWithoutBarrelPassthrough(t *testing.T) {
	t.Parallel()
	sp := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"acme"},
	}, map[string]string{"/node_modules/acme/dist/index.d.ts": `export * as Button from "./Button.ts"`})

	fix := makeBarrelFix("acme/Private", "/node_modules/acme/dist/Private.d.ts", "secret")
	if result := sp.Apply(makeExport("acme", "acme/Private", ""), fix); result != fix {
		t.Errorf("expected fix for a module no barrel forwards to be returned unchanged, got %#v", result)
	}
}

func TestApplyBarrelRewriteFromBarrelPassthrough(t *testing.T) {
	t.Parallel()
	sp := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"effect"},
	}, map[string]string{"/node_modules/effect/dist/index.d.ts": `export * as Function from "./Function.js"`})

	for _, tt := range []struct{ moduleSpecifier, moduleFileName string }{
		{"effect", "/node_modules/effect/dist/index.d.ts"},
		{"effect/testing", "/node_modules/effect/dist/testing/index.d.ts"},
		{"effect/http", "/node_modules/effect/dist/http/index.d.ts"},
	} {
		export := makeExport("effect", tt.moduleSpecifier, "effect/Function")
		export.Syntax = autoimport.ExportSyntaxNamed
		fix := makeBarrelFix(tt.moduleSpecifier, tt.moduleFileName, "pipe")
		if result := sp.Apply(export, fix); result != fix {
			t.Errorf("%s: expected fix from a barrel to be returned unchanged, got %#v", tt.moduleSpecifier, result)
		}
	}
}

func TestApplyBarrelRewriteWithAlias(t *testing.T) {
	t.Parallel()
	sp := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"@effect/platform"},
		ImportAliases:        map[string]string{"@effect/platform": "Platform"},
	}, map[string]string{"/node_modules/@effect/platform/dist/dts/index.d.ts": `export * as HttpClient from "./HttpClient.js"`})

	export := makeExport("@effect/platform", "@effect/platform/HttpClient", "")
	fix := makeBarrelFix("@effect/platform/HttpClient", "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts", "request")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.Name != "request" {
		t.Errorf("expected name 'request' (original export name), got %q", result.Name)
	}
	if result.NamespacePrefix != "Platform" {
		t.Errorf("expected namespace prefix 'Platform', got %q", result.NamespacePrefix)
	}
}

func TestApplyNamespaceRewriteWithoutUsagePositionFallsBack(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")
	fix.UsagePosition = nil

	result := sp.Apply(export, fix)
	if result != fix {
		t.Error("expected unchanged fix when usage position is unavailable")
	}
}

func TestApplyBarrelRewriteWithoutUsagePositionFallsBack(t *testing.T) {
	t.Parallel()
	sp := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"@effect/platform"},
	}, map[string]string{"/node_modules/@effect/platform/dist/dts/index.d.ts": `export * as HttpClient from "./HttpClient.js"`})

	export := makeExport("@effect/platform", "@effect/platform/HttpClient", "")
	fix := makeBarrelFix("@effect/platform/HttpClient", "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts", "request")
	fix.UsagePosition = nil

	result := sp.Apply(export, fix)
	if result != fix {
		t.Error("expected unchanged fix when usage position is unavailable")
	}
}

func TestApplyTopLevelReexportIgnore(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
		TopLevelNamedReexports:  etscore.TopLevelNamedReexportsIgnore, // "ignore"
	})

	// Export is a reexport (target module differs from module)
	export := makeExport("effect", "effect", "effect/Effect")
	export.Syntax = autoimport.ExportSyntaxNamed
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	} else {
		// When ignoring reexports, the fix should be returned unchanged
		if result.ImportKind != lsproto.ImportKindNamed {
			t.Errorf("expected ImportKindNamed (unchanged), got %v", result.ImportKind)
		}
		if result.Name != "succeed" {
			t.Errorf("expected name 'succeed' (unchanged), got %q", result.Name)
		}
	}
}

func TestApplyTopLevelReexportFollow(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
		TopLevelNamedReexports:  etscore.TopLevelNamedReexportsFollow, // "follow"
	})

	// Export is a reexport (target module differs from module)
	export := makeExport("effect", "effect", "effect/Effect")
	export.Syntax = autoimport.ExportSyntaxNamed
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect", "succeed")

	result := sp.Apply(export, fix)
	// When following reexports, the reexport fix should be suppressed (nil)
	// so the direct submodule namespace import wins instead
	if result != nil {
		t.Errorf("expected nil result (suppressed reexport), got fix with ImportKind=%v", result.ImportKind)
	}
}

func TestApplyNoMatchPassthrough(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("lodash", "lodash", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "lodash", "map")

	result := sp.Apply(export, fix)
	if result != fix {
		t.Error("expected fix to be returned unchanged (same pointer)")
	}
}

func TestApplyNonAddNewPassthrough(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("effect", "effect/Effect", "")
	fix := &autoimport.Fix{
		AutoImportFix: &lsproto.AutoImportFix{
			Kind:            lsproto.AutoImportFixKindAddToExisting,
			ImportKind:      lsproto.ImportKindNamed,
			ModuleSpecifier: "effect/Effect",
			Name:            "succeed",
		},
	}

	result := sp.Apply(export, fix)
	if result != fix {
		t.Error("expected non-AddNew fix to be returned unchanged")
	}
}

func TestApplyCaseInsensitiveMatching(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"Effect"},
	})

	// Package name is lowercase but config was uppercase
	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	} else if result.ImportKind != lsproto.ImportKindNamespace {
		t.Errorf("expected case-insensitive match to trigger namespace rewrite, got %v", result.ImportKind)
	}
}

func TestApplyNilInputs(t *testing.T) {
	t.Parallel()
	sp := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	})

	if result := sp.Apply(nil, makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")); result == nil {
		t.Error("expected fix returned when export is nil")
	}

	export := makeExport("effect", "effect/Effect", "")
	if result := sp.Apply(export, nil); result != nil {
		t.Error("expected nil when fix is nil")
	}
}

func TestNewFixTransformerNilForEmptyPrefs(t *testing.T) {
	t.Parallel()
	transformer := NewFixTransformer(&etscore.ResolvedEffectPluginOptions{}, nil, nil)
	if transformer != nil {
		t.Error("expected nil transformer for empty preferences")
	}
}

func TestNewFixTransformerAppliesPolicy(t *testing.T) {
	t.Parallel()
	transformer := NewFixTransformer(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	}, nil, nil)
	if transformer == nil {
		t.Fatal("expected non-nil transformer")
	}

	export := makeExport("effect", "effect/Effect", "")
	fixes := []*autoimport.Fix{makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")}

	result := transformer(export, fixes)
	if len(result) != 1 {
		t.Fatalf("expected 1 fix, got %d", len(result))
	}
	if result[0].ImportKind != lsproto.ImportKindNamespace {
		t.Errorf("expected ImportKindNamespace, got %v", result[0].ImportKind)
	}
}

func TestNewFixTransformerPrefersExistingNamespaceUse(t *testing.T) {
	t.Parallel()
	transformer := NewFixTransformer(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
	}, nil, nil)
	if transformer == nil {
		t.Fatal("expected non-nil transformer")
	}

	export := makeExport("effect", "effect/testing/TestClock", "")
	useNamespace := &autoimport.Fix{AutoImportFix: &lsproto.AutoImportFix{
		Kind:            lsproto.AutoImportFixKindUseNamespace,
		ImportKind:      lsproto.ImportKindNamespace,
		ModuleSpecifier: "effect/testing/TestClock",
		Name:            "testClockWith",
		NamespacePrefix: "TestClock",
	}}
	addToExisting := makeAddNewFix(lsproto.ImportKindNamed, "effect/testing/TestClock", "testClockWith")
	addToExisting.Kind = lsproto.AutoImportFixKindAddToExisting

	result := transformer(export, []*autoimport.Fix{useNamespace, addToExisting})
	if len(result) != 1 {
		t.Fatalf("expected 1 fix, got %d", len(result))
	}
	if result[0].Kind != lsproto.AutoImportFixKindUseNamespace {
		t.Fatalf("expected UseNamespace fix, got %v", result[0].Kind)
	}
}
