// refactor: 4:22-4:23, 6:22-6:23, 8:10-8:11, 12:10-12:11, 16:10-16:11
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
