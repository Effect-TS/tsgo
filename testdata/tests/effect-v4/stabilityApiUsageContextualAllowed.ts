// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [{
      "name": "@effect/language-service",
      "overrides": [
        { "include": ["emptyEntry.ts"], "options": { "allowedExperimentalApis": [""] } },
        { "include": ["package.ts"], "options": { "allowedExperimentalApis": ["effect-v4-tests"] } },
        { "include": ["module.ts"], "options": { "allowedExperimentalApis": ["effect-v4-tests/api"] } },
        { "include": ["export.ts"], "options": { "allowedExperimentalApis": ["effect-v4-tests/api#Reportable"] } },
        { "include": ["otherExport.ts"], "options": { "allowedExperimentalApis": ["effect-v4-tests/api#other"] } }
      ]
    }]
  }
}

// @filename: api.ts
/** @stability stable */
export interface Reportable {
  /** @stability experimental */
  readonly test?: number
}

// @filename: emptyEntry.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning
import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })

// @filename: package.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning
import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })

// @filename: module.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning
import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })

// @filename: export.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning
import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })

// @filename: otherExport.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning
import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })
