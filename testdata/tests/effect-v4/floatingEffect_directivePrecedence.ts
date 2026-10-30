import { Effect } from "effect"

// reported as error: no directive applies yet
Effect.succeed("default")

// @effect-diagnostics floatingEffect:off
Effect.succeed("section off")
// @effect-diagnostics-next-line floatingEffect:error
Effect.succeed("next-line error overrides the section")
Effect.succeed("section off again after the next-line directive")

// @effect-diagnostics unnecessaryEffectGen:warning
Effect.succeed("other rule section keeps floatingEffect off")

// @effect-diagnostics FLOATINGEFFECT:warning
Effect.succeed("case-insensitive section sets warning")

// @effect-diagnostics-next-line *:off
Effect.succeed("wildcard next-line off")
Effect.succeed("warning again after the wildcard next-line directive")

// @effect-diagnostics floatingEffect:message
Effect.succeed("message section")

// @effect-diagnostics *:off
// @effect-diagnostics floatingEffect:suggestion
Effect.succeed("rule section after wildcard off re-enables as suggestion")

// @effect-diagnostics *:error
Effect.succeed("wildcard section overrides the rule section")
