// @effect-diagnostics *:off
// @effect-diagnostics preferEffectRecord:warning
export {}
declare global {
  interface Object extends Pick<ObjectConstructor, "keys"> {}
}
Object.keys({ a: 1 })
