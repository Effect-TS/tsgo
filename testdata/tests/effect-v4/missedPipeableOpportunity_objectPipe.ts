// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "pipeableMinArgCount": 2
      }
    ]
  }
}

// @filename: missedPipeableOpportunity_objectPipe.ts
// @effect-diagnostics missedPipeableOpportunity:warning
interface HasPipe {
  pipe(): unknown
}

declare global {
  interface Object extends HasPipe {}
}

declare const g: (x: object) => object
declare const f: (x: object) => number

// Should report: object literals get `pipe` from a base interface of the augmented Object
export const fromObject = f(g({ a: 1 }))
