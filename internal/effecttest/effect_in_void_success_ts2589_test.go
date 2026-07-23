package effecttest

import "testing"

func TestIssue381EffectInVoidSuccessDoesNotReportExcessiveTypeDepth(t *testing.T) {
	t.Parallel()

	withoutPlugin := collectDiagnosticStringsFromContent(t, buildIssue381EffectInVoidSuccessCase(false))
	if hasDiagnosticCode(withoutPlugin, "TS2589:") {
		t.Fatalf("did not expect TS2589 without plugin, got %v", withoutPlugin)
	}

	withPlugin := collectDiagnosticStringsFromContent(t, buildIssue381EffectInVoidSuccessCase(true))
	if hasDiagnosticCode(withPlugin, "TS2589:") {
		t.Fatalf("did not expect effectInVoidSuccess to introduce TS2589, got %v", withPlugin)
	}
}

func buildIssue381EffectInVoidSuccessCase(pluginEnabled bool) string {
	pluginConfig := ""
	if pluginEnabled {
		pluginConfig = `,
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "effectInVoidSuccess": "warning"
        }
      }
    ]`
	}

	return `// @filename: tsconfig.json
{
  "compilerOptions": {
    "target": "ES2025",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "noEmit": true,
    "skipLibCheck": true,
    "strict": true` + pluginConfig + `
  }
}

// @filename: test.ts
import type { Schema } from "effect"

type Recurse<Value> = {
  [Key in keyof Value]: Recurse<Value[Key]>
}

type Json = readonly Json[] | JsonObject
type JsonObject = { readonly [key: string]: Json }

type ValueType<Definition> =
  Definition extends Schema.Codec<infer Value, Json> ? Value : never

declare let value: Recurse<
  ValueType<Schema.Codec<JsonObject>> | null
>

value = null
`
}
