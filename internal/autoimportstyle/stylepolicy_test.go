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

func TestApplyNamespaceRewrite(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name            string
		options         *etscore.ResolvedEffectPluginOptions
		moduleSpecifier string
		exportName      string
		fixKind         lsproto.AutoImportFixKind
		wantPrefix      string
	}{
		{
			name:            "add new",
			options:         &etscore.ResolvedEffectPluginOptions{NamespaceImportPackages: []string{"effect"}},
			moduleSpecifier: "effect/Effect",
			exportName:      "succeed",
			fixKind:         lsproto.AutoImportFixKindAddNew,
			wantPrefix:      "Effect",
		},
		{
			name:            "add to existing",
			options:         &etscore.ResolvedEffectPluginOptions{NamespaceImportPackages: []string{"effect"}},
			moduleSpecifier: "effect/testing/TestClock",
			exportName:      "testClockWith",
			fixKind:         lsproto.AutoImportFixKindAddToExisting,
			wantPrefix:      "TestClock",
		},
		{
			name: "alias",
			options: &etscore.ResolvedEffectPluginOptions{
				NamespaceImportPackages: []string{"effect"},
				ImportAliases:           map[string]string{"effect": "Fx"},
			},
			moduleSpecifier: "effect/Effect",
			exportName:      "succeed",
			fixKind:         lsproto.AutoImportFixKindAddNew,
			wantPrefix:      "Fx",
		},
		{
			name:            "case-insensitive package",
			options:         &etscore.ResolvedEffectPluginOptions{NamespaceImportPackages: []string{"Effect"}},
			moduleSpecifier: "effect/Effect",
			exportName:      "succeed",
			fixKind:         lsproto.AutoImportFixKindAddNew,
			wantPrefix:      "Effect",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			fix := makeAddNewFix(lsproto.ImportKindNamed, tt.moduleSpecifier, tt.exportName)
			fix.Kind = tt.fixKind
			result := newStylePolicy(tt.options).Apply(makeExport("effect", tt.moduleSpecifier, ""), fix)
			if result == nil {
				t.Fatal("expected non-nil result")
			}
			if result.Kind != lsproto.AutoImportFixKindAddNew || result.ImportKind != lsproto.ImportKindNamespace {
				t.Errorf("expected AddNew namespace import, got kind %v import kind %v", result.Kind, result.ImportKind)
			}
			if result.Name != tt.exportName || result.ModuleSpecifier != tt.moduleSpecifier || result.NamespacePrefix != tt.wantPrefix {
				t.Errorf("got name %q specifier %q prefix %q, want %q %q %q", result.Name, result.ModuleSpecifier, result.NamespacePrefix, tt.exportName, tt.moduleSpecifier, tt.wantPrefix)
			}
		})
	}
}

func TestApplyBarrelRewrite(t *testing.T) {
	t.Parallel()
	platformDisk := map[string]string{"/node_modules/@effect/platform/dist/dts/index.d.ts": `export * as HttpClient from "./HttpClient.js"`}
	acmeDisk := map[string]string{
		"/node_modules/acme/dist/index.d.ts":       `export * as Http from "./HttpClient.ts"`,
		"/node_modules/acme/dist/forms/index.d.ts": `export * as TextInput from "./TextInput.ts"`,
	}
	tests := []struct {
		name            string
		options         *etscore.ResolvedEffectPluginOptions
		disk            map[string]string
		packageName     string
		moduleSpecifier string
		moduleFileName  string
		wantBarrel      string
		wantPrefix      string
	}{
		{
			name:            "package root barrel",
			options:         &etscore.ResolvedEffectPluginOptions{BarrelImportPackages: []string{"@effect/platform"}},
			disk:            platformDisk,
			packageName:     "@effect/platform",
			moduleSpecifier: "@effect/platform/HttpClient",
			moduleFileName:  "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
			wantBarrel:      "@effect/platform",
			wantPrefix:      "HttpClient",
		},
		{
			name: "alias",
			options: &etscore.ResolvedEffectPluginOptions{
				BarrelImportPackages: []string{"@effect/platform"},
				ImportAliases:        map[string]string{"@effect/platform": "Platform"},
			},
			disk:            platformDisk,
			packageName:     "@effect/platform",
			moduleSpecifier: "@effect/platform/HttpClient",
			moduleFileName:  "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts",
			wantBarrel:      "@effect/platform",
			wantPrefix:      "Platform",
		},
		{
			name:            "export name differs from module",
			options:         &etscore.ResolvedEffectPluginOptions{BarrelImportPackages: []string{"acme"}},
			disk:            acmeDisk,
			packageName:     "acme",
			moduleSpecifier: "acme/HttpClient",
			moduleFileName:  "/node_modules/acme/dist/HttpClient.d.ts",
			wantBarrel:      "acme",
			wantPrefix:      "Http",
		},
		{
			name:            "nearest forwarding barrel",
			options:         &etscore.ResolvedEffectPluginOptions{BarrelImportPackages: []string{"acme"}},
			disk:            acmeDisk,
			packageName:     "acme",
			moduleSpecifier: "acme/forms/TextInput",
			moduleFileName:  "/node_modules/acme/dist/forms/TextInput.d.ts",
			wantBarrel:      "acme/forms",
			wantPrefix:      "TextInput",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sp := newBarrelPolicy(tt.options, tt.disk)
			result := sp.Apply(makeExport(tt.packageName, tt.moduleSpecifier, ""), makeBarrelFix(tt.moduleSpecifier, tt.moduleFileName, "request"))
			if result == nil {
				t.Fatal("expected non-nil result")
			}
			if result.ImportKind != lsproto.ImportKindNamed || result.Name != "request" {
				t.Errorf("expected named import of 'request', got import kind %v name %q", result.ImportKind, result.Name)
			}
			if result.ModuleSpecifier != tt.wantBarrel || result.NamespacePrefix != tt.wantPrefix {
				t.Errorf("got specifier %q prefix %q, want %q %q", result.ModuleSpecifier, result.NamespacePrefix, tt.wantBarrel, tt.wantPrefix)
			}
		})
	}
}

func TestApplyPassthrough(t *testing.T) {
	t.Parallel()
	namespacePolicy := newStylePolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"effect"},
		TopLevelNamedReexports:  etscore.TopLevelNamedReexportsIgnore,
	})
	platformPolicy := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"@effect/platform"},
	}, map[string]string{"/node_modules/@effect/platform/dist/dts/index.d.ts": `export * as HttpClient from "./HttpClient.js"`})
	acmePolicy := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"acme"},
	}, map[string]string{"/node_modules/acme/dist/index.d.ts": `export * as Button from "./Button.ts"`})
	effectBarrelPolicy := newBarrelPolicy(&etscore.ResolvedEffectPluginOptions{
		BarrelImportPackages: []string{"effect"},
	}, map[string]string{"/node_modules/effect/dist/index.d.ts": `export * as Function from "./Function.js"`})

	withoutUsage := func(fix *autoimport.Fix) *autoimport.Fix {
		fix.UsagePosition = nil
		return fix
	}
	withKind := func(fix *autoimport.Fix, kind lsproto.AutoImportFixKind) *autoimport.Fix {
		fix.Kind = kind
		return fix
	}
	namedReexport := func(moduleID string, targetModuleID string) *autoimport.Export {
		export := makeExport("effect", moduleID, targetModuleID)
		export.Syntax = autoimport.ExportSyntaxNamed
		return export
	}

	tests := []struct {
		name   string
		policy *stylePolicy
		export *autoimport.Export
		fix    *autoimport.Fix
	}{
		{"package without style", namespacePolicy, makeExport("lodash", "lodash", ""), makeAddNewFix(lsproto.ImportKindNamed, "lodash", "map")},
		{"use namespace fix", namespacePolicy, makeExport("effect", "effect/Effect", ""), withKind(makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed"), lsproto.AutoImportFixKindUseNamespace)},
		{"namespace without usage position", namespacePolicy, makeExport("effect", "effect/Effect", ""), withoutUsage(makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed"))},
		{"ignored top-level reexport", namespacePolicy, namedReexport("effect", "effect/Effect"), makeAddNewFix(lsproto.ImportKindNamed, "effect", "succeed")},
		{"barrel without usage position", platformPolicy, makeExport("@effect/platform", "@effect/platform/HttpClient", ""), withoutUsage(makeBarrelFix("@effect/platform/HttpClient", "/node_modules/@effect/platform/dist/dts/HttpClient.d.ts", "request"))},
		{"module no barrel forwards", acmePolicy, makeExport("acme", "acme/Private", ""), makeBarrelFix("acme/Private", "/node_modules/acme/dist/Private.d.ts", "secret")},
		{"from barrel effect", effectBarrelPolicy, namedReexport("effect", "effect/Function"), makeBarrelFix("effect", "/node_modules/effect/dist/index.d.ts", "pipe")},
		{"from barrel effect/testing", effectBarrelPolicy, namedReexport("effect/testing", "effect/Function"), makeBarrelFix("effect/testing", "/node_modules/effect/dist/testing/index.d.ts", "pipe")},
		{"from barrel effect/http", effectBarrelPolicy, namedReexport("effect/http", "effect/Function"), makeBarrelFix("effect/http", "/node_modules/effect/dist/http/index.d.ts", "pipe")},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			if result := tt.policy.Apply(tt.export, tt.fix); result != tt.fix {
				t.Errorf("expected fix to be returned unchanged, got %#v", result)
			}
		})
	}
}

func TestApplySuppressed(t *testing.T) {
	t.Parallel()
	followReexport := makeExport("effect", "effect", "effect/Effect")
	followReexport.Syntax = autoimport.ExportSyntaxNamed
	tests := []struct {
		name    string
		options *etscore.ResolvedEffectPluginOptions
		export  *autoimport.Export
		fix     *autoimport.Fix
	}{
		{
			name:    "effect internal module",
			options: &etscore.ResolvedEffectPluginOptions{NamespaceImportPackages: []string{"effect"}},
			export:  makeExport("effect", "effect/internal/schema/parser", ""),
			fix:     makeAddNewFix(lsproto.ImportKindNamed, "effect/internal/schema/parser", "succeed"),
		},
		{
			name: "followed top-level reexport",
			options: &etscore.ResolvedEffectPluginOptions{
				NamespaceImportPackages: []string{"effect"},
				TopLevelNamedReexports:  etscore.TopLevelNamedReexportsFollow,
			},
			export: followReexport,
			fix:    makeAddNewFix(lsproto.ImportKindNamed, "effect", "succeed"),
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			if result := newStylePolicy(tt.options).Apply(tt.export, tt.fix); result != nil {
				t.Errorf("expected fix to be suppressed, got %#v", result)
			}
		})
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
