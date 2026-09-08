// @effect-diagnostics *:off
// @effect-diagnostics nodeBuiltinImport:warning
import { Effect } from "effect"
import { randomUUID } from "node:crypto"
import fs from "node:fs"

export const preview = Effect.sync(() => ({
  id: randomUUID(),
  contents: fs.readFileSync("config.json", "utf8")
}))
