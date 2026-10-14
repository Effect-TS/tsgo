// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}

// @filename: test.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// reported: namespace re-exported by the barrel
import { Effect } from "effect"
import { Predicate as P } from "effect"
import { HttpClient } from "effect/http"
import { TestClock } from "effect/testing"

// reported: declared in the imported module
import { map } from "effect/Array"
import { some as makeSome } from "effect/Option"

// reported: type-only imports
import type { Layer } from "effect"
import { type Context } from "effect"
import type { Exit } from "effect/Exit"

// not reported: named re-export with topLevelNamedReexports "ignore"
import { pipe } from "effect"
import { flow as composeFlow } from "effect"

// not reported: already namespace imports
import * as Duration from "effect/Duration"
import * as Schedule from "effect/Schedule"

// not reported: packages outside namespaceImportPackages
import { it } from "@effect/vitest"
import * as VitestUtils from "@effect/vitest/utils"

export const usages = [
  Effect, P, HttpClient, TestClock, map, makeSome, pipe, composeFlow, Duration, Schedule, it, VitestUtils
]
export type Usages = [Layer.Layer<never>, Context.Context<never>, Exit<void>]
