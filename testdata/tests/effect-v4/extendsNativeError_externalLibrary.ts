// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": { "extendsNativeError": "error" }
    }]
  }
}

// @filename: /node_modules/problem/package.json
{ "name": "problem", "version": "1.0.0", "types": "index.ts" }

// @filename: /node_modules/problem/index.ts
export class ExternalProblem extends Error {}

// @filename: main.ts
import { ExternalProblem } from "problem"

export class LocalProblem extends Error {}
export const value = ExternalProblem
