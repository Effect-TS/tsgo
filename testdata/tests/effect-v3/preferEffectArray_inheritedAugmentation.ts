// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferEffectArray:warning
export {}
declare global {
  interface Array<T> extends Pick<ArrayConstructor, "isArray"> {}
}
Array.isArray([])
