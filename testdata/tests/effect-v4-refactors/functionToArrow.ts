// refactor: 5:20-5:25, 12:20-12:25, 21:5-21:9
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
