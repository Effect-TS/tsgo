// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "anyUnknownInErrorContext": "error",
          "effectInFailure": "error"
        }
      }
    ]
  }
}

// @filename: taggedTemplateSymbolInterpolation_ts2731.ts
const self: unique symbol = Symbol("self")
function sql(strings: TemplateStringsArray, ...args: Array<unknown>): string {
  return strings.join("?") + args.length
}
export const q = sql`(${self}) < 1`
