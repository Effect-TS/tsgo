// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning

/** @stability unstable */
export const unstableValue = 1

/** @stability experimental */
export const experimentalValue = 2

/** @stability stable */
export const stableValue = 3

export const unstableUse = unstableValue
export const experimentalUse = experimentalValue
export const stableUse = stableValue
const alias = unstableValue
export const aliasedUse = alias

const object = {
  /** @stability experimental */
  member: 1
}
export const memberUse = object.member

/** @stability experimental */
export function overloaded(value: string): string
export function overloaded(value: number): number
export function overloaded(value: string | number): string | number {
  return value
}

export const experimentalOverload = overloaded("hello")
export const stableOverload = overloaded(1)
