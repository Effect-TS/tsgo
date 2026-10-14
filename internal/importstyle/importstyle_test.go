package importstyle

import (
	"testing"

	"github.com/effect-ts/tsgo/etscore"
)

func TestPolicyStyle(t *testing.T) {
	t.Parallel()
	policy := NewPolicy(&etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages: []string{"Effect", "shared"},
		BarrelImportPackages:    []string{"@effect/platform", "shared"},
	})
	tests := []struct {
		specifier string
		expected  Style
	}{
		{"effect", StyleNamespace},
		{"effect/Effect", StyleNamespace},
		{"EFFECT/Array", StyleNamespace},
		{"@effect/platform", StyleBarrel},
		{"@effect/platform/HttpClient", StyleBarrel},
		{"shared/Thing", StyleNamespace},
		{"lodash", StyleNone},
		{"./effect", StyleNone},
		{"", StyleNone},
	}
	for _, tt := range tests {
		if got := policy.Style(tt.specifier); got != tt.expected {
			t.Errorf("Style(%q) = %d, want %d", tt.specifier, got, tt.expected)
		}
	}
}

func TestPolicyIsEmpty(t *testing.T) {
	t.Parallel()
	if !NewPolicy(nil).IsEmpty() {
		t.Error("nil options should produce an empty policy")
	}
	if !NewPolicy(&etscore.ResolvedEffectPluginOptions{}).IsEmpty() {
		t.Error("no packages should produce an empty policy")
	}
	if NewPolicy(&etscore.ResolvedEffectPluginOptions{BarrelImportPackages: []string{"effect"}}).IsEmpty() {
		t.Error("a barrel package should produce a non-empty policy")
	}
}

func TestPackageName(t *testing.T) {
	t.Parallel()
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
		if got := PackageName(tt.input); got != tt.expected {
			t.Errorf("PackageName(%q) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

func TestNamespaceName(t *testing.T) {
	t.Parallel()
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
		if got := NamespaceName(tt.input); got != tt.expected {
			t.Errorf("NamespaceName(%q) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

func TestReexportSpecifier(t *testing.T) {
	t.Parallel()
	tests := []struct {
		barrel   string
		relative string
		expected string
	}{
		{"effect", "./Effect.ts", "effect/Effect"},
		{"effect", "./Function.js", "effect/Function"},
		{"effect/http", "./HttpClient.d.ts", "effect/http/HttpClient"},
		{"@effect/platform", "./HttpClient", "@effect/platform/HttpClient"},
		{"effect", "./internal/core.ts", ""},
		{"effect", "../Effect.ts", ""},
		{"effect", "./index.js", ""},
		{"effect", "other", ""},
		{"", "./Effect.ts", ""},
	}
	for _, tt := range tests {
		if got := ReexportSpecifier(tt.barrel, tt.relative); got != tt.expected {
			t.Errorf("ReexportSpecifier(%q, %q) = %q, want %q", tt.barrel, tt.relative, got, tt.expected)
		}
	}
}
