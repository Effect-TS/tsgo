// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [{ "name": "@effect/language-service" }]
  }
}

// @filename: stabilityApiUsageDefault.ts
/** @stability unstable */
export const unstableValue = 1

/** @stability experimental */
export const experimentalValue = 2

export const unstableUse = unstableValue
export const experimentalUse = experimentalValue
