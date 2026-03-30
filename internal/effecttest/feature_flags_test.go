package effecttest_test

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestEffectDiagnosticsDisabled(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnostics": false,
        "diagnosticSeverity": {
          "floatingEffect": "error"
        }
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"

Effect.succeed(1)`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyDiagnostics(t, nil)
}

func TestEffectRefactorsDisabled(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "refactors": false
      }
    ]
  }
}
// @Filename: /test.ts
import * as Eff from "effect/Effect"

export const program = () =>
  [|Eff.gen(function*() {
    const a = yield* Eff.succeed(1)
    const b = yield* Eff.succeed(2)
    return a + b
  })|]`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	ranges := f.Ranges()
	if len(ranges) != 1 {
		t.Fatalf("expected exactly one range, got %d", len(ranges))
	}
	actions := f.GetRefactorActionsForRange(t, lsconv.FileNameToDocumentURI(ranges[0].FileName()), ranges[0].LSRange)
	if len(actions) != 0 {
		t.Fatalf("expected no refactor actions when refactors=false, got %d", len(actions))
	}
}

func TestEffectOverridesCanDisableMatchingFile(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error"
        },
        "overrides": [
          {
            "include": ["src/**/*.ts"],
            "options": {
              "diagnosticSeverity": {
                "floatingEffect": "off"
              }
            }
          }
        ]
      }
    ]
  }
}
// @Filename: /src/test.ts
import { Effect } from "effect"

Effect.succeed(1)`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyDiagnostics(t, nil)
}

func TestEffectOverridesCanOverrideRuleOptions(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "missedPipeableOpportunity": "error"
        },
        "pipeableMinArgCount": 2,
        "overrides": [
          {
            "include": ["src/**/*.ts"],
            "options": {
              "pipeableMinArgCount": 3
            }
          }
        ]
      }
    ]
  }
}
// @Filename: /src/test.ts
import { identity, Schema } from "effect"

const User = Schema.Struct({ id: Schema.Number })
export const preview = identity(identity(Schema.decodeEffect(User)({ id: 1 })))`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyDiagnostics(t, nil)
}

func TestEffectOverridesPreserveExtendedConfigBasePaths(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /configs/base.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "overrides": [
          {
            "include": ["shared/**/*.ts"],
            "options": {
              "diagnosticSeverity": {
                "floatingEffect": "off"
              }
            }
          }
        ]
      }
    ]
  }
}
// @Filename: /tsconfig.json
{
  "extends": "./configs/base.json",
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error"
        }
      }
    ]
  }
}
// @Filename: /configs/shared/test.ts
import { Effect } from "effect"

Effect.succeed(1)`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyDiagnostics(t, nil)
}
