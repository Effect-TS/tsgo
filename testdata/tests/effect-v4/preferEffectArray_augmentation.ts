// @effect-diagnostics *:off
// @effect-diagnostics preferEffectArray:warning
export {}
declare global {
  interface Array<T> {
    map(callbackfn: (value: T) => string): string[]
  }
  interface ReadonlyArray<T> {
    map(callbackfn: (value: T) => string): string[]
  }
}
const mutable = [1, 2]
const readonly: readonly number[] = mutable
mutable.map(n => n + 1)
readonly.map(n => n + 1)
mutable.filter(n => n > 1)
readonly.filter(n => n > 1)
