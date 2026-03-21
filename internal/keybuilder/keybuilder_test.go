package keybuilder

import (
	"testing"

	"github.com/effect-ts/effect-typescript-go/etscore"
)

func TestCyrb53(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input    string
		expected string
	}{
		{
			input:    "@effect/harness-effect-v4/ExpectedServiceIdentifier",
			expected: "fc438e0396fbb4f7",
		},
		{
			input:    "@effect/harness-effect-v4/ErrorA",
			expected: "0e8acd2c08314dfd",
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			t.Parallel()
			got := Cyrb53(tt.input)
			if got != tt.expected {
				t.Errorf("Cyrb53(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

func TestCyrb53Length(t *testing.T) {
	t.Parallel()
	inputs := []string{"", "hello", "test/path/file", "@effect/myapp/services/AuthService"}
	for _, input := range inputs {
		got := Cyrb53(input)
		if len(got) != 16 {
			t.Errorf("Cyrb53(%q) produced %q with length %d, want 16", input, got, len(got))
		}
	}
}

func TestCreateString(t *testing.T) {
	t.Parallel()
	defaultServicePattern := []etscore.KeyPattern{
		{Target: "service", Pattern: "default", SkipLeadingPath: []string{"src/"}},
	}
	defaultErrorPattern := []etscore.KeyPattern{
		{Target: "error", Pattern: "default", SkipLeadingPath: []string{"src/"}},
	}
	defaultHashedServicePattern := []etscore.KeyPattern{
		{Target: "service", Pattern: "default-hashed", SkipLeadingPath: []string{"src/"}},
	}
	packageIdentifierServicePattern := []etscore.KeyPattern{
		{Target: "service", Pattern: "package-identifier", SkipLeadingPath: []string{"src/"}},
	}

	tests := []struct {
		name             string
		sourceFileName   string
		packageName      string
		packageDirectory string
		className        string
		target           string
		keyPatterns      []etscore.KeyPattern
		expected         string
	}{
		{
			name:             "default pattern - file at package root, class different from file",
			sourceFileName:   "/.src/test.ts",
			packageName:      "@effect/harness-effect-v4",
			packageDirectory: "/.src",
			className:        "ExpectedServiceIdentifier",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "@effect/harness-effect-v4/test/ExpectedServiceIdentifier",
		},
		{
			name:             "default pattern - error target",
			sourceFileName:   "/.src/test.ts",
			packageName:      "@effect/harness-effect-v4",
			packageDirectory: "/.src",
			className:        "ErrorA",
			target:           "error",
			keyPatterns:      defaultErrorPattern,
			expected:         "@effect/harness-effect-v4/test/ErrorA",
		},
		{
			name:             "package-identifier pattern - omits subdirectory",
			sourceFileName:   "/.src/test.ts",
			packageName:      "@effect/harness-effect-v4",
			packageDirectory: "/.src",
			className:        "ExpectedServiceIdentifier",
			target:           "service",
			keyPatterns:      packageIdentifierServicePattern,
			expected:         "@effect/harness-effect-v4/test/ExpectedServiceIdentifier",
		},
		{
			name:             "default-hashed pattern produces 16-char hex",
			sourceFileName:   "/.src/test.ts",
			packageName:      "@effect/harness-effect-v4",
			packageDirectory: "/.src",
			className:        "ExpectedServiceIdentifier",
			target:           "service",
			keyPatterns:      defaultHashedServicePattern,
			expected:         Cyrb53("@effect/harness-effect-v4/test/ExpectedServiceIdentifier"),
		},
		{
			name:             "cyrb53 cross-check with known value",
			sourceFileName:   "/.src/test.ts",
			packageName:      "@effect/harness-effect-v4",
			packageDirectory: "/.src",
			className:        "ExpectedServiceIdentifier",
			target:           "service",
			keyPatterns: []etscore.KeyPattern{
				{Target: "service", Pattern: "default", SkipLeadingPath: []string{"src/"}},
			},
			expected: "@effect/harness-effect-v4/test/ExpectedServiceIdentifier",
		},
		{
			name:             "class name matches file name - omits class name",
			sourceFileName:   "/project/src/AuthService.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "AuthService",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "@effect/myapp/AuthService",
		},
		{
			name:             "class name matches file name case-insensitive - omits class name, uses file casing",
			sourceFileName:   "/project/src/authservice.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "AuthService",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "@effect/myapp/authservice",
		},
		{
			name:             "index file name is stripped",
			sourceFileName:   "/project/src/services/index.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "AuthService",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "@effect/myapp/services/AuthService",
		},
		{
			name:             "skipLeadingPath strips src/ prefix from subdirectory",
			sourceFileName:   "/project/src/services/AuthService.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "AuthService",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "@effect/myapp/services/AuthService",
		},
		{
			name:             "subdirectory preserved when not matching skipLeadingPath",
			sourceFileName:   "/project/lib/services/AuthService.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "MyService",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "@effect/myapp/lib/services/AuthService/MyService",
		},
		{
			name:             "package-identifier omits subdirectory",
			sourceFileName:   "/project/lib/services/AuthService.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "MyService",
			target:           "service",
			keyPatterns:      packageIdentifierServicePattern,
			expected:         "@effect/myapp/AuthService/MyService",
		},
		{
			name:             "no matching target - returns empty",
			sourceFileName:   "/project/src/file.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "Foo",
			target:           "error",
			keyPatterns:      defaultServicePattern,
			expected:         "",
		},
		{
			name:             "empty package name - returns empty",
			sourceFileName:   "/project/src/file.ts",
			packageName:      "",
			packageDirectory: "/project",
			className:        "Foo",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "",
		},
		{
			name:             "source file not under package directory - returns empty (skips)",
			sourceFileName:   "/other/file.ts",
			packageName:      "@effect/myapp",
			packageDirectory: "/project",
			className:        "Foo",
			target:           "service",
			keyPatterns:      defaultServicePattern,
			expected:         "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			got := CreateString(tt.sourceFileName, tt.packageName, tt.packageDirectory, tt.className, tt.target, tt.keyPatterns)
			if got != tt.expected {
				t.Errorf("CreateString(%q, %q, %q, %q, %q, ...) = %q, want %q",
					tt.sourceFileName, tt.packageName, tt.packageDirectory, tt.className, tt.target, got, tt.expected)
			}
		})
	}
}
