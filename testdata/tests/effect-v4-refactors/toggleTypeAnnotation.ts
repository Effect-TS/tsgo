// refactor: 4:14-4:15, 5:14-5:15, 8:14-8:15, 11:14-11:15, 13:7-13:8, 14:7-14:8, 17:10-17:11, 18:10-18:11, 19:3-19:4, 20:3-20:4
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
