// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "effectFn": ["span", "suggested-span", "inferred-span", "no-span", "untraced"]
      }
    ]
  }
}

// @filename: effectFnOpportunity_invalid.ts
// @effect-v3

// Generator functions
export const generatorExpression = function*() {
  yield 1
  yield 2
}

export function* generatorDeclaration() {
  yield 1
  yield 2
}

// Async functions
export const asyncArrow = async () => {
  return 42
}

export const asyncFunctionExpression = async function() {
  return 42
}

export async function asyncDeclaration() {
  return 42
}
