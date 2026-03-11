import { Effect } from "effect"

// These should trigger floatingEffect diagnostic (code 377001)
Effect.succeed("floating")
Effect.never
Effect.void
Effect.fail("error")

// These should NOT trigger (assigned to variable)
const ok = Effect.succeed(1)
const x = Effect.void

// These should NOT trigger (assignment expressions)
let logicalAssignment: Effect.Effect<void, never, never>
logicalAssignment = Effect.void
logicalAssignment ??= Effect.void
logicalAssignment ||= Effect.void
logicalAssignment &&= Effect.void

// Property assignment should NOT trigger
class MyClass {
  boot: Effect.Effect<void, never, never>
  constructor() {
    this.boot = Effect.void
  }
}
