// refactor: 5:22-5:23, 7:22-7:23, 9:10-9:11, 13:10-13:11, 17:10-17:11
// @effect-v3
import * as T from "effect/Effect"

export const test1 = () => T.succeed(42)

export const test2 = () => (true ? T.succeed(42) : false)

function sillyGenerics<A>(value: A) {
  return T.fail(value)
}

function removeAnnotation(): number {
  return 42
}

function removeAnnotationWithSpace(): number {
  return 42
}
