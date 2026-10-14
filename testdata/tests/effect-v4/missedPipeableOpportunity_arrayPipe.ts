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

// @filename: missedPipeableOpportunity_arrayPipe.ts
// @effect-diagnostics missedPipeableOpportunity:warning
interface HasPipe {
  pipe(): unknown
}

declare global {
  interface Array<T> extends HasPipe {}
}

declare const g: (x: ReadonlyArray<number>) => object
declare const h: (x: object) => object
declare const f: (x: object) => number

// Should report: array literals get `pipe` from a base interface of the augmented Array
export const fromArray = f(g([1, 2, 3]))

// Should not report: object literals do not inherit Array members
export const fromObject = f(h({ a: 1 }))
