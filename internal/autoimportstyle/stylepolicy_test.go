package autoimportstyle

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/ls/autoimport"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

func TestNewStylePolicy(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"effect", "Effect"},
		BarrelImportPackages:    []string{"@effect/platform"},
		ImportAliases:           map[string]string{"effect": "Fx"},
	})

	if !sp.namespacePackages["effect"] {
		t.Error("expected 'effect' in namespace packages")
	}
	if sp.namespacePackages["Effect"] {
		t.Error("expected case-insensitive key (lowercase) for 'Effect'")
	}
	if !sp.barrelPackages["@effect/platform"] {
		t.Error("expected '@effect/platform' in barrel packages")
	}
	if sp.aliases["effect"] != "Fx" {
		t.Errorf("expected alias 'Fx' for 'effect', got %q", sp.aliases["effect"])
	}
}

func TestStylePolicyIsEmpty(t *testing.T) {
	var nilPolicy *stylePolicy
	if !nilPolicy.isEmpty() {
		t.Error("nil policy should be empty")
	}

	empty := newStylePolicy(StylePreferences{})
	if !empty.isEmpty() {
		t.Error("empty preferences should produce empty policy")
	}

	nonEmpty := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"effect"},
	})
	if nonEmpty.isEmpty() {
		t.Error("non-empty preferences should not be empty")
	}
}

func makeExport(packageName string, moduleID autoimport.ModuleID, targetModuleID autoimport.ModuleID) *autoimport.Export {
	return &autoimport.Export{
		ExportID: autoimport.ExportID{
			ModuleID:   moduleID,
			ExportName: "succeed",
		},
		PackageName: packageName,
		Target: autoimport.ExportID{
			ModuleID: targetModuleID,
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
	sp := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"effect"},
	})

	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
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

func TestApplyNamespaceRewriteWithAlias(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"effect"},
		ImportAliases:           map[string]string{"effect": "Fx"},
	})

	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
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

func TestApplyBarrelRewrite(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
		BarrelImportPackages: []string{"@effect/platform"},
	})

	export := makeExport("@effect/platform", "@effect/platform/HttpClient", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "@effect/platform/HttpClient", "request")

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

func TestApplyBarrelRewriteWithAlias(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
		BarrelImportPackages: []string{"@effect/platform"},
		ImportAliases:        map[string]string{"@effect/platform": "Platform"},
	})

	export := makeExport("@effect/platform", "@effect/platform/HttpClient", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "@effect/platform/HttpClient", "request")

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
	sp := newStylePolicy(StylePreferences{
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
	sp := newStylePolicy(StylePreferences{
		BarrelImportPackages: []string{"@effect/platform"},
	})

	export := makeExport("@effect/platform", "@effect/platform/HttpClient", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "@effect/platform/HttpClient", "request")
	fix.UsagePosition = nil

	result := sp.Apply(export, fix)
	if result != fix {
		t.Error("expected unchanged fix when usage position is unavailable")
	}
}

func TestApplyTopLevelReexportIgnore(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"effect"},
		FollowTopLevelReexports: false, // "ignore"
	})

	// Export is a reexport (target module differs from module)
	export := makeExport("effect", "effect", "effect/Effect")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	// When ignoring reexports, the fix should be returned unchanged
	if result.ImportKind != lsproto.ImportKindNamed {
		t.Errorf("expected ImportKindNamed (unchanged), got %v", result.ImportKind)
	}
	if result.Name != "succeed" {
		t.Errorf("expected name 'succeed' (unchanged), got %q", result.Name)
	}
}

func TestApplyTopLevelReexportFollow(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"effect"},
		FollowTopLevelReexports: true, // "follow"
	})

	// Export is a reexport (target module differs from module)
	export := makeExport("effect", "effect", "effect/Effect")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect", "succeed")

	result := sp.Apply(export, fix)
	// When following reexports, the reexport fix should be suppressed (nil)
	// so the direct submodule namespace import wins instead
	if result != nil {
		t.Errorf("expected nil result (suppressed reexport), got fix with ImportKind=%v", result.ImportKind)
	}
}

func TestApplyNoMatchPassthrough(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
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
	sp := newStylePolicy(StylePreferences{
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
	sp := newStylePolicy(StylePreferences{
		NamespaceImportPackages: []string{"Effect"},
	})

	// Package name is lowercase but config was uppercase
	export := makeExport("effect", "effect/Effect", "")
	fix := makeAddNewFix(lsproto.ImportKindNamed, "effect/Effect", "succeed")

	result := sp.Apply(export, fix)
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.ImportKind != lsproto.ImportKindNamespace {
		t.Errorf("expected case-insensitive match to trigger namespace rewrite, got %v", result.ImportKind)
	}
}

func TestApplyNilInputs(t *testing.T) {
	sp := newStylePolicy(StylePreferences{
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

func TestPackageNameFromSpecifier(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"effect", "effect"},
		{"effect/Effect", "effect"},
		{"@scope/pkg", "@scope/pkg"},
		{"@scope/pkg/sub", "@scope/pkg"},
		{"@scope/pkg/sub/deep", "@scope/pkg"},
		{"./local", ""},
		{"/absolute", ""},
		{"", ""},
		{"@scope", "@scope"},
	}

	for _, tt := range tests {
		result := packageNameFromSpecifier(tt.input)
		if result != tt.expected {
			t.Errorf("packageNameFromSpecifier(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestInferNamespaceName(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"effect/Effect", "Effect"},
		{"effect", "effect"},
		{"@scope/pkg/Foo", "Foo"},
		{"@effect/platform/HttpClient", "HttpClient"},
		{"", ""},
		{"foo/", ""},
	}

	for _, tt := range tests {
		result := inferNamespaceName(tt.input)
		if result != tt.expected {
			t.Errorf("inferNamespaceName(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestNewFixTransformerNilForEmptyPrefs(t *testing.T) {
	transformer := NewFixTransformer(StylePreferences{})
	if transformer != nil {
		t.Error("expected nil transformer for empty preferences")
	}
}

func TestNewFixTransformerAppliesPolicy(t *testing.T) {
	transformer := NewFixTransformer(StylePreferences{
		NamespaceImportPackages: []string{"effect"},
	})
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
