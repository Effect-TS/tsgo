// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
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

// @filename: test.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// reported: named re-export followed to the declaring module
import { pipe } from "effect"
import { flow as composeFlow } from "effect"
import { type hole } from "effect"

// reported: unchanged by topLevelNamedReexports
import { Effect } from "effect"
import { identity } from "effect/Function"

// not reported
import * as Option from "effect/Option"

export const usages = [pipe, composeFlow, Effect, identity, Option]
export type Usages = typeof hole
