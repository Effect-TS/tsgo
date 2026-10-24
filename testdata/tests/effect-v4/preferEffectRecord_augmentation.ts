// @effect-diagnostics *:off
// @effect-diagnostics preferEffectRecord:warning
import * as R from "effect/Record"

declare global {
  interface ObjectConstructor { keys<T>(value: T): (keyof T)[] }
  interface Object { hasOwnProperty(key: string): boolean }
}
const data = { a: 1 }
Object.keys(data)
Object["keys"](data)
data.hasOwnProperty("a")
Object.prototype.hasOwnProperty.call(data, "a")
Object.values(data)
Object.hasOwn(data, "a")
R.values(data)
R.has(data, "a")
