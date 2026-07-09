import { Effect } from "effect"

// This should trigger (no directive)
Effect.succeed(true)

// This should be suppressed by next-line directive (JSDoc style)
/** @effect-diagnostics-next-line floatingEffect:off */
Effect.succeed(42)

// This should trigger (directive only affects previous line)
Effect.succeed(false)

// This should be suppressed by next-line directive (comment style)
// @effect-diagnostics-next-line floatingEffect:off
Effect.succeed(42)

// This should trigger again
Effect.succeed("world")

// Section directive: set to warning for rest of file
/** @effect-diagnostics floatingEffect:warning */
Effect.succeed(1)
Effect.succeed(2)

// Section directive: turn off for rest of file
/** @effect-diagnostics floatingEffect:off */
Effect.succeed(3)
Effect.succeed(4)
