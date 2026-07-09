// refactor: 5:36-5:37
// @effect-v3
import { Effect } from "effect"

export class Asd extends Effect.Service<Asd>()("Asd", {
  succeed: {}
}) {}
