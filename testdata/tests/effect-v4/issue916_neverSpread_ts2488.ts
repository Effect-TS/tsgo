// @filename: tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "promiseInEffectSuccess": "error"
        }
      }
    ]
  }
}

// @filename: package.json
{ "type": "module" }

// @filename: issue916_neverSpread_ts2488.ts
declare function g(...values: Array<number>): void

export function f() {
  const rows = [1] as never;
  g(...rows);
  return [...rows];
}
