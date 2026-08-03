import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Command from "effect/unstable/cli/Command"
import { describe, expect, it } from "vitest"
import { ensureIntegrationSelected, integrationFlags } from "../src/cli/integrationFlags.js"

const parseIntegrationFlags = async (args: ReadonlyArray<string>) => {
  let result: { readonly typescript: boolean; readonly oxlint: boolean } | undefined
  const command = Command.make("patch", integrationFlags).pipe(
    Command.withHandler((flags) => Effect.sync(() => result = flags))
  )
  await Effect.runPromise(
    Command.runWith(command, { version: "test" })(args).pipe(Effect.provide(NodeServices.layer))
  )
  return result
}

describe("integration flags", () => {
  it("patches only TypeScript by default", async () => {
    await expect(parseIntegrationFlags([])).resolves.toEqual({ typescript: true, oxlint: false })
  })

  it("adds Oxlint without disabling TypeScript", async () => {
    await expect(parseIntegrationFlags(["--oxlint"]))
      .resolves.toEqual({ typescript: true, oxlint: true })
  })

  it("supports selecting only Oxlint", async () => {
    await expect(parseIntegrationFlags(["--no-typescript", "--oxlint"]))
      .resolves.toEqual({ typescript: false, oxlint: true })
  })

  it("rejects disabling every integration", async () => {
    await expect(Effect.runPromise(ensureIntegrationSelected(false, false)))
      .rejects.toThrow("Select at least one integration")
  })
})
