// refactor: 5:14-5:15, 6:14-6:15, 9:14-9:15, 12:14-12:15, 14:7-14:8, 15:7-15:8, 18:10-18:11, 19:10-19:11, 20:3-20:4, 21:3-21:4
// @effect-v3
import * as T from "effect/Effect"

export const test1 = T.succeed
export const test2 = T.fail("LOL")

const predefined = 42
export const test3 = predefined

const callable = () => 42
export const test4 = callable

const removeAnnotation: number = 42
const removeAnnotationWithSpace: number = 42

class Test {
  static liveAdd = "hello"
  static liveRemove: string = "hello"
  propAdd = "hello"
  propRemove: string = "hello"
}
