package effecttest_test

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"

	_ "github.com/effect-ts/effect-typescript-go/etslshooks"
	_ "github.com/effect-ts/effect-typescript-go/etstesthooks"
)

func TestAutoImportEffectStyleConsistency_namespace(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["EFFECT"]
      }
    ]
  }
}
// @Filename: /node_modules/effect/package.json
{
  "name": "effect",
  "version": "0.0.0"
}
// @Filename: /node_modules/effect/Effect.ts
export const succeed = <A>(value: A): A => value;
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	completion := "completion"

	f.VerifyApplyCodeActionFromCompletion(t, &completion, &fourslash.ApplyCodeActionFromCompletionOptions{
		Name:        "succeed",
		Source:      "effect/Effect",
		Description: "Add import from \"effect/Effect\"",
		NewFileContent: new(`import * as Effect from "effect/Effect";

Effect.succeed(1);`),
		UserPreferences: preferences,
	})

	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as Effect from "effect/Effect";

Effect.succeed(1);
`}, preferences)
}

func TestAutoImportEffectStyleConsistency_barrel(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["@EFFECT/PLATFORM"]
      }
    ]
  }
}
// @Filename: /node_modules/@effect/platform/package.json
{
  "name": "@effect/platform",
  "version": "0.0.0"
}
// @Filename: /node_modules/@effect/platform/HttpClient.ts
export const request = (url: string): string => url;
// @Filename: /node_modules/@effect/platform/index.ts
export * as HttpClient from "./HttpClient";
// @Filename: /mainCompletion.ts
request/*completion*/("/");
// @Filename: /mainFix.ts
request/*fix*/("/");
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	completion := "completion"

	// After barrel rewrite, the module specifier changes to the barrel package
	f.VerifyApplyCodeActionFromCompletion(t, &completion, &fourslash.ApplyCodeActionFromCompletionOptions{
		Name:        "request",
		Source:      "@effect/platform",
		Description: "Add import from \"@effect/platform\"",
		NewFileContent: new(`import { HttpClient } from "@effect/platform";

HttpClient.request("/");`),
		UserPreferences: preferences,
	})

	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import { HttpClient } from "@effect/platform";

HttpClient.request("/");
`}, preferences)
}

func TestAutoImportEffectStyleConsistency_topLevelNamedReexportsIgnore(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "topLevelNamedReexports": "ignore"
      }
    ]
  }
}
// @Filename: /node_modules/effect/package.json
{
  "name": "effect",
  "version": "0.0.0"
}
// @Filename: /node_modules/effect/Effect.ts
export const succeed = <A>(value: A): A => value;
// @Filename: /node_modules/effect/index.ts
export { succeed } from "./Effect";
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}
	completion := "completion"

	// With topLevelNamedReexports="ignore", the reexport from "effect" is kept as named import.
	// The completion picks the best available fix, which is the named import from the barrel.
	f.VerifyApplyCodeActionFromCompletion(t, &completion, &fourslash.ApplyCodeActionFromCompletionOptions{
		Name:        "succeed",
		Source:      "effect",
		Description: "Add import from \"effect\"",
		NewFileContent: new(`import { succeed } from "effect";

succeed(1);`),
		UserPreferences: preferences,
	})

	f.GoToMarker(t, "fix")
	// Two import fixes available: named from "effect" (reexport kept) and namespace from "effect/Effect"
	f.VerifyImportFixAtPosition(t, []string{
		`import { succeed } from "effect";

succeed(1);
`,
		`import * as Effect from "effect/Effect";

Effect.succeed(1);
`,
	}, preferences)
}

func TestAutoImportEffectStyleConsistency_topLevelNamedReexportsFollow(t *testing.T) {
	t.Parallel()
	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "topLevelNamedReexports": "follow"
      }
    ]
  }
}
// @Filename: /node_modules/effect/package.json
{
  "name": "effect",
  "version": "0.0.0"
}
// @Filename: /node_modules/effect/Effect.ts
export const succeed = <A>(value: A): A => value;
// @Filename: /node_modules/effect/index.ts
export { succeed } from "./Effect";
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	preferences := &lsutil.UserPreferences{
		IncludeCompletionsForModuleExports:    core.TSTrue,
		IncludeCompletionsForImportStatements: core.TSTrue,
	}

	// With topLevelNamedReexports="follow", the reexport from "effect" is suppressed,
	// leaving only the direct namespace import from "effect/Effect".
	f.GoToMarker(t, "fix")
	f.VerifyImportFixAtPosition(t, []string{`import * as Effect from "effect/Effect";

Effect.succeed(1);
`}, preferences)
}
