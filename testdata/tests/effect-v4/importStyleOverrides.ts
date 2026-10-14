// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "overrides": [{
          "include": ["legacy/**/*.ts"],
          "options": {
            "namespaceImportPackages": [],
            "barrelImportPackages": ["effect"]
          }
        }, {
          "include": ["follow.ts"],
          "options": { "topLevelNamedReexports": "follow" }
        }]
      }
    ]
  }
}

// @filename: app.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning
// @effect-diagnostics preferBarrelImport:warning

// reported by preferNamespaceImport: the base config applies
import { Effect } from "effect"
// not reported: pipe is a named re-export and the base config keeps "ignore"
import { pipe } from "effect"
// not reported: barrelImportPackages is only set for legacy files
import * as Record from "effect/Record"

export const app = pipe(Effect.void, Effect.as(Record.empty()))

// @filename: legacy/old.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning
// @effect-diagnostics preferBarrelImport:warning

// not reported: the override clears namespaceImportPackages
import { Effect } from "effect"
// reported by preferBarrelImport: the override sets barrelImportPackages
import * as Record from "effect/Record"

export const old = Effect.as(Effect.void, Record.empty())

// @filename: follow.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// reported: the override sets topLevelNamedReexports to "follow"
import { pipe } from "effect"
import * as Effect from "effect/Effect"

export const followed = pipe(Effect.void, Effect.asVoid)
