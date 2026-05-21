// @effect-diagnostics *:off
// @effect-diagnostics lazyEffect:warning
import { Effect } from "effect"

export interface PreviewService {
  preview: () => Effect.Effect<void>
}
