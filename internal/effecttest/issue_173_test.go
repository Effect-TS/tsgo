package effecttest_test

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestIssue173_DirectYieldedThisDoesNotReportFalsePositiveTS2683(t *testing.T) {
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
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /repro.ts
import { Effect, Exit, Scope } from "effect"

class Repro {
  readonly #scope: Scope.Closeable = Scope.makeUnsafe()

  run(): Effect.Effect<void> {
    return Effect.gen({ self: this }, function* () {
      yield* Scope.close([|this|].#scope, Exit.void)
    })
  }
}`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyNonSuggestionDiagnostics(t, nil)
}

func TestIssue173_HoistedThisScopeDoesNotReportTS2683(t *testing.T) {
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
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /repro.ts
import { Effect, Exit, Scope } from "effect"

class Repro {
  readonly #scope: Scope.Closeable = Scope.makeUnsafe()

  run(): Effect.Effect<void> {
    return Effect.gen({ self: this }, function* () {
      const scope = this.#scope
      yield* Scope.close(scope, Exit.void)
    })
  }
}`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyNonSuggestionDiagnostics(t, nil)
}
