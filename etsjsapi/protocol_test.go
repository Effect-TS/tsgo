package etsjsapi

import (
	"encoding/json"
	"testing"
)

func TestRunEffectDiagnosticsParamsOverrideEffectOptions(t *testing.T) {
	t.Parallel()

	t.Run("absent", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts"}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OverrideEffectOptions != nil {
			t.Fatal("expected absent settings to preserve tsconfig fallback")
		}
	})

	t.Run("defaults", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts","overrideEffectOptions":{}}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OverrideEffectOptions == nil {
			t.Fatal("expected settings")
		}
		if !params.OverrideEffectOptions.Diagnostics || !params.OverrideEffectOptions.Refactors {
			t.Fatal("expected Effect option defaults")
		}
		if params.OverrideEffectOptions.DiagnosticSeverity == nil {
			t.Fatal("expected default diagnostic severities")
		}
	})

	t.Run("explicit false", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts","overrideEffectOptions":{"refactors":false}}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OverrideEffectOptions == nil || params.OverrideEffectOptions.Refactors {
			t.Fatal("expected explicit false setting")
		}
	})

	t.Run("key patterns", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts","overrideEffectOptions":{"keyPatterns":[{"target":"custom","pattern":"default-hashed","skipLeadingPath":["lib/"]}]}}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OverrideEffectOptions == nil || len(params.OverrideEffectOptions.KeyPatterns) != 1 {
			t.Fatal("expected one key pattern")
		}
		pattern := params.OverrideEffectOptions.KeyPatterns[0]
		if pattern.Target != "custom" || pattern.Pattern != "default-hashed" || len(pattern.SkipLeadingPath) != 1 || pattern.SkipLeadingPath[0] != "lib/" {
			t.Fatalf("unexpected key pattern: %+v", pattern)
		}
	})

	t.Run("null", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts","overrideEffectOptions":null}`), &params); err == nil {
			t.Fatal("expected null settings to be rejected")
		}
	})
}

func TestRunEffectDiagnosticsParamsOnlyRules(t *testing.T) {
	t.Parallel()

	t.Run("omitted", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts"}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OnlyRules != nil {
			t.Fatal("expected omitted onlyRules to use configured rules")
		}
	})

	t.Run("null", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts","onlyRules":null}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OnlyRules != nil {
			t.Fatal("expected null onlyRules to use configured rules")
		}
	})

	t.Run("empty", func(t *testing.T) {
		t.Parallel()

		var params RunEffectDiagnosticsParams
		if err := json.Unmarshal([]byte(`{"targetFilePath":"test.ts","onlyRules":[]}`), &params); err != nil {
			t.Fatal(err)
		}
		if params.OnlyRules == nil || len(*params.OnlyRules) != 0 {
			t.Fatal("expected empty onlyRules to select no rules")
		}
	})
}
