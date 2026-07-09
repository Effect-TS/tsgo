// @effect-v3
// refactor: 6:20-6:25, 13:20-13:25, 22:5-22:9
/**
 * Docs
 */
export function myTest(name: string) {
  return name.length
}

/**
 * Docs
 */
export function myTest2(name: string) {
  if (name === "LOL") return 42
  return name.length
}

class Sample {
  /**
   * Docs
   */
  test(name: string) {
    return name.length
  }
}
